pub mod context;
pub mod prompt;

use crate::consts::FILE_EXT_RHAI;
use crate::error::CtGenError;
use crate::profile::{CtGenProfile, CtGenProfileConfigOverrides, CtGenPrompt, CtGenTarget};
use crate::task::context::CtGenTaskContext;
use crate::task::prompt::{CtGenRenderedPrompt, CtGenTaskPrompt};
use crate::CtGen;
use anyhow::Result;
use database_reflection::adapter::mariadb_innodb::MariadbInnodbReflectionAdapter;
use database_reflection::adapter::reflection_adapter::{Connected, ReflectionAdapter, ReflectionAdapterUninitialized};
use handlebars::{handlebars_helper, DirectorySourceOptions, Handlebars};
use handlebars_concat::HandlebarsConcat;
use handlebars_inflector::HandlebarsInflector;
use serde_json::{json, Value};
use sqlx::MySql;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::slice::Iter;
use std::str::FromStr;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::join;
use tokio::process::Command;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct CtGenTask<'a> {
    profile: CtGenProfile,
    overrides: Option<CtGenProfileConfigOverrides>,
    prompts: Vec<CtGenTaskPrompt>,
    prompt_answers: HashMap<String, Value>,

    reflection_adapter: MariadbInnodbReflectionAdapter<Connected<MySql>>,
    table: Option<String>,
    context_dir: String,
    target_dir: String,

    context: Option<CtGenTaskContext>,
    renderer: Handlebars<'a>,
}

impl CtGenTask<'_> {
    pub async fn new(
        profile: &CtGenProfile,
        context_dir: &str,
        table: Option<&str>,
        profile_overrides: Option<CtGenProfileConfigOverrides>,
    ) -> Result<Self> {
        let config = profile.configuration();
        let overrides = profile_overrides.as_ref();

        let env_file = if let Some(overrides) = overrides {
            if let Some(env_file) = overrides.env_file() {
                env_file.to_string()
            } else {
                config.env_file().to_string()
            }
        } else {
            config.env_file().to_string()
        };

        let env_var = if let Some(overrides) = overrides {
            if let Some(env_var) = overrides.env_var() {
                env_var.to_string()
            } else {
                config.env_var().to_string()
            }
        } else {
            config.env_var().to_string()
        };

        let dsn = if let Some(overrides) = overrides {
            if let Some(dsn) = overrides.dsn() {
                dsn.to_string()
            } else {
                config.dsn().to_string()
            }
        } else {
            config.dsn().to_string()
        };

        let target_dir = if let Some(overrides) = overrides {
            if let Some(target_dir) = overrides.target_dir() {
                target_dir.to_string()
            } else {
                config.target_dir().to_string()
            }
        } else {
            config.target_dir().to_string()
        };

        // determine dsn, validate env-file, env-var and dsn properties
        let dsn = if dsn.is_empty() {
            // no dsn, env-file and env-var must be valid

            if env_file.is_empty() {
                return Err(CtGenError::ValidationError(
                    "Invalid env-file specified. Either valid DSN or valid env-file is required.".to_string(),
                )
                .into());
            }

            dotenvy::from_filename(env_file).map_err(|e| CtGenError::ValidationError(format!("Invaid env file specified: {}", e)))?;

            if env_var.is_empty() {
                return Err(CtGenError::ValidationError(
                    "Invalid env-var specified. Either valid DSN or valid env-file and env-var is required.".to_string(),
                )
                .into());
            }

            env::var(env_var).map_err(|e| CtGenError::ValidationError(format!("Invaid env var specified: {}", e)))?
        } else {
            dsn
        };

        // validate target dir existence and write permissions
        // target dir should be relative to context dir,
        // combining the two and resolving canonical path should yield an existing path
        let canonical_target_dir = if target_dir.is_empty() || target_dir == "." {
            context_dir.to_string()
        } else {
            let target_fullpath = CtGen::get_filepath(context_dir, &target_dir);

            CtGen::init_config_dir(&target_fullpath).await?;

            target_fullpath
        };

        let (b1, b2) = join!(
            CtGen::file_exists(&canonical_target_dir),
            CtGen::file_is_writable(&canonical_target_dir)
        );

        if !b1 || !b2 {
            return Err(CtGenError::ValidationError("Invalid target-dir specified.".to_string()).into());
        }

        // prepare context data
        let reflection_adapter = MariadbInnodbReflectionAdapter::new(&dsn).connect().await?;

        // prepare prompts
        let mut prompts: Vec<CtGenTaskPrompt> = Vec::new();

        let mut pre_create_context = true;

        if reflection_adapter.get_database_name().is_empty() {
            // dsn has no database name, must add prompt
            prompts.push(CtGenTaskPrompt::PromptDatabase);
            pre_create_context = false;
        }

        if table.is_none() {
            // no task subject given, must add prompt
            prompts.push(CtGenTaskPrompt::PromptTable);
            pre_create_context = false;
        } else {
            // check if table exists
            let table = table.unwrap().to_string();
            let tables = reflection_adapter.list_table_names().await?;
            if !tables.contains(&table) {
                return Err(CtGenError::ValidationError("Table does not exist".to_string()).into());
            }
        }

        for prompt_name in profile.prompts() {
            prompts.push(CtGenTaskPrompt::PromptGeneric {
                prompt_id: prompt_name.to_string(),
                prompt_data: profile.prompt(prompt_name).unwrap().clone(),
            });
        }

        // prepare context
        let mut context: Option<CtGenTaskContext> = None;

        if pre_create_context {
            let database = reflection_adapter.get_reflection().await?;

            context = Some(CtGenTaskContext::new(database, table.unwrap())?);
        }

        // init renderer
        let mut handlebars = Handlebars::new();

        handlebars.register_templates_directory(profile.templates_dir(), DirectorySourceOptions::default())?;

        let scripts_dir = profile.scripts_dir();
        let walker = WalkDir::new(&scripts_dir);
        let scripts_dir_iter = walker
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok().map(|e| e.into_path()))
            .filter(|tpl_path| tpl_path.to_string_lossy().ends_with(FILE_EXT_RHAI))
            .filter(|tpl_path| {
                tpl_path
                    .file_stem()
                    .map(|stem| !stem.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
            })
            .filter_map(|script_path| {
                script_path
                    .strip_prefix(&scripts_dir)
                    .ok()
                    .map(|script_canonical_name| {
                        let script_name = script_canonical_name
                            .components()
                            .map(|component| component.as_os_str().to_string_lossy())
                            .collect::<Vec<_>>()
                            .join("/");

                        script_name.strip_suffix(FILE_EXT_RHAI).map(|s| s.to_owned()).unwrap_or(script_name)
                    })
                    .map(|script_canonical_name| (script_canonical_name, script_path))
            });

        for (script_canonical_name, script_path) in scripts_dir_iter {
            handlebars.register_script_helper_file(&script_canonical_name, script_path)?;
        }

        handlebars.register_helper("concat", Box::new(HandlebarsConcat));
        handlebars.register_helper("inflect", Box::new(HandlebarsInflector));

        handlebars_helper!(json: |input: Value| serde_json::to_string(&input).unwrap_or(String::from("{}")));
        handlebars.register_helper("json", Box::new(json));

        Ok(CtGenTask {
            profile: profile.clone(),
            overrides: profile_overrides,
            prompts,
            prompt_answers: HashMap::new(),
            reflection_adapter,
            table: table.map(str::to_string),
            context_dir: context_dir.to_string(),
            target_dir: canonical_target_dir,
            context,
            renderer: handlebars,
        })
    }

    /// Template profile
    pub fn profile(&self) -> &CtGenProfile {
        &self.profile
    }

    /// Get profile override properties
    pub fn overrides(&self) -> Option<&CtGenProfileConfigOverrides> {
        self.overrides.as_ref()
    }

    /// Reflection adapter
    pub fn reflection_adapter(&self) -> &MariadbInnodbReflectionAdapter<Connected<MySql>> {
        &self.reflection_adapter
    }

    /// Task subject
    pub fn table(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// Canonical context directory
    pub fn context_dir(&self) -> &str {
        &self.context_dir
    }

    /// Canonical target directory
    pub fn target_dir(&self) -> &str {
        &self.target_dir
    }

    /// List of prompts in order of appearance
    pub fn prompts(&self) -> Iter<'_, CtGenTaskPrompt> {
        self.prompts.iter()
    }

    /// List of unanswered prompts in order of appearance
    pub fn prompts_unanswered(&self) -> Vec<CtGenTaskPrompt> {
        self.prompts
            .iter()
            .filter(|p| match p {
                CtGenTaskPrompt::PromptGeneric { prompt_id, prompt_data: _ } => !self.prompt_answers.contains_key(prompt_id),
                CtGenTaskPrompt::PromptDatabase => self.reflection_adapter.get_database_name().is_empty(),
                CtGenTaskPrompt::PromptTable => self.table().is_none(),
            })
            .cloned()
            .collect::<Vec<CtGenTaskPrompt>>()
    }

    /// Get prompt answer by prompt id
    pub fn prompt_answer(&self, prompt: &str) -> Option<&Value> {
        self.prompt_answers.get(prompt)
    }

    /// Get all prompt answers
    pub fn prompt_answers(&self) -> std::collections::hash_map::Iter<'_, String, Value> {
        self.prompt_answers.iter()
    }

    /// Save prompt answers and prepare context data
    pub async fn set_prompt_answer(&mut self, prompt: &CtGenTaskPrompt, answer: Value) -> Result<()> {
        match prompt {
            CtGenTaskPrompt::PromptDatabase => {
                self.reflection_adapter
                    .set_database_name(answer.as_str().unwrap_or_default())
                    .await?;
            }
            CtGenTaskPrompt::PromptTable => {
                let tables = self.reflection_adapter.list_table_names().await?;
                if tables.contains(&answer.as_str().unwrap_or_default().to_string()) {
                    self.table = Some(answer.as_str().unwrap_or_default().to_string());
                } else {
                    return Err(CtGenError::ValidationError("Table does not exist".to_string()).into());
                }
            }
            CtGenTaskPrompt::PromptGeneric { prompt_id, prompt_data } => {
                if prompt_data.required() {
                    // check answer validity before accepting
                    if let Value::String(s) = answer.clone() {
                        if s.trim().is_empty() {
                            return Err(CtGenError::ValidationError(format!("Invalid answer to prompt {}", prompt_id)).into());
                        }
                    } else if let Value::Array(ar) = answer.clone() {
                        if ar.is_empty() {
                            return Err(CtGenError::ValidationError(format!("Invalid answer to prompt {}", prompt_id)).into());
                        }
                    }
                }

                self.prompt_answers.insert(prompt_id.to_string(), answer);
            }
        }

        self.update_context().await?;

        Ok(())
    }

    /// Make sure every prompt answer is sent to the context
    async fn update_context(&mut self) -> Result<()> {
        if let Some(context) = self.context.as_mut() {
            for (prompt_id, prompt_answer) in self.prompt_answers.iter() {
                context.set_prompt_answer(prompt_id, prompt_answer);
            }
        } else if !self.reflection_adapter.get_database_name().is_empty() && self.table.is_some() {
            let database = self.reflection_adapter.get_reflection().await?;
            self.context = Some(CtGenTaskContext::new(database, &self.table.clone().unwrap())?);
        }

        Ok(())
    }

    /// Check if we are ready to render shit
    pub fn is_context_ready(&self) -> bool {
        self.context.is_some() && self.prompts_unanswered().is_empty()
    }

    /// Direct rendering
    pub fn render(&self, template_content: &str) -> Result<String> {
        Ok(self.renderer.render_template(template_content, &self.context)?)
    }

    /// Template rendering
    pub fn render_template(&self, template_name: &str) -> Result<String> {
        Ok(self.renderer.render(template_name, &self.context)?)
    }

    /// Render target by target template and target output file
    pub async fn render_target(&self, target: &CtGenTarget) -> Result<()> {
        let output = self.render_template(target.template())?;

        let target_file = if target.target().contains("{{") && target.target().contains("}}") {
            self.render(target.target())? // there could be variables in the target
        } else {
            target.target().to_string() // target is a literal
        };

        // full canonical path to output file
        let canonical_target_file = CtGen::get_filepath(self.target_dir(), &target_file);

        // init sub-directories if necessary
        CtGen::init_config_dir(Path::new(&canonical_target_file).parent().unwrap().to_string_lossy().as_ref()).await?;

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&canonical_target_file)
            .await?;
        file.write_all(output.as_bytes()).await?;
        file.flush().await?;

        // run formatter, if defined
        if let Some(formatter) = target.formatter() {
            let rendered_formatter = self
                .renderer
                .render_template(formatter, &json!({"target": &canonical_target_file}))?;

            let output = if cfg!(target_os = "windows") {
                Command::new("cmd").args(["/C", &rendered_formatter]).output().await?
            } else {
                Command::new("sh").arg("-c").arg(&rendered_formatter).output().await?
            };

            if !output.status.success() {
                // TODO handle formatter error
            }

            let formatter_output = String::from_utf8_lossy(&output.stdout);

            // TODO handle formatter output better
            println!("Target {} formatter output: {}", target.target() , formatter_output);
        }

        Ok(())
    }

    /// Render all targets and write the output files
    pub async fn run(&self) -> Result<()> {
        if !self.is_context_ready() {
            return Err(CtGenError::RuntimeError("Context not ready to run all render tasks.".to_string()).into());
        }

        for target_name in self.profile.targets() {
            if let Some(target) = self.profile.target(target_name) {
                if let Some(condition) = target.condition() {
                    let evaluated_condition = self.render(condition)?;

                    if evaluated_condition.trim() != "1" {
                        break;
                    }
                }

                self.render_target(target).await?;
            }
        }

        Ok(())
    }

    /// Render all elements of a prompt and yield a new owned prompt
    pub fn render_prompt(&self, prompt: &CtGenPrompt) -> Result<CtGenRenderedPrompt> {
        // if condition property is set, evaluate it to decide whether to proceed with the prompt
        let condition = if let Some(condition) = prompt.condition() {
            self.render(condition).ok()
        } else {
            None
        };

        // render prompt text
        let prompt_text = self.render(prompt.prompt())?;

        // render options if defined as string
        let options = if prompt.options().is_str() {
            // template expression that needs to be evaluated and exploded by ","
            let options = self
                .render(prompt.options().as_str().unwrap())?
                .split(',')
                .map(str::to_string)
                .collect::<Vec<String>>();

            Value::from(options)
        } else {
            Value::from_str(&serde_json::to_string(prompt.options())?)?
        };

        let condition_met = condition.is_none() || condition.is_some_and(|s| s.trim() == "1");

        Ok(CtGenRenderedPrompt::new(condition_met, prompt_text, options, prompt.multiple()))
    }

    /// Get context data
    pub fn context(&self) -> Option<&CtGenTaskContext> {
        self.context.as_ref()
    }

    /// Get renderer instance
    pub fn renderer(&self) -> &Handlebars<'_> {
        &self.renderer
    }
}
