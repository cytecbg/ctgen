pub mod context;
pub mod prompt;

use crate::error::CtGenError;
use crate::profile::{CtGenProfile, CtGenProfileConfigOverrides};
use crate::task::context::CtGenTaskContext;
use crate::task::prompt::CtGenTaskPrompt;
use crate::CtGen;
use anyhow::Result;
use database_reflection::adapter::mariadb_innodb::MariadbInnodbReflectionAdapter;
use database_reflection::adapter::reflection_adapter::{Connected, ReflectionAdapter, ReflectionAdapterUninitialized};
use serde_json::Value;
use sqlx::MySql;
use std::collections::HashMap;
use std::env;
use std::slice::Iter;
use tokio::join;

#[derive(Debug)]
pub struct CtGenTask {
    profile: CtGenProfile,
    overrides: Option<CtGenProfileConfigOverrides>,
    prompts: Vec<CtGenTaskPrompt>,
    prompt_answers: HashMap<String, Value>,

    reflection_adapter: MariadbInnodbReflectionAdapter<Connected<MySql>>,
    table: Option<String>,
    context_dir: String,
    target_dir: String,

    context: Option<CtGenTaskContext>,
}

impl CtGenTask {
    pub async fn new(
        profile: &CtGenProfile,
        context_dir: &str,
        table: Option<&String>,
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

        // // validate templates dir existence and read permissions
        // let canonical_templates_dir =
        //     if profile.configuration().templates_dir().is_empty() || profile.configuration().templates_dir() == "." {
        //         profile.context_dir().to_string() // profile context_dir is not the same as task context_dir
        //     } else {
        //         CtGen::get_filepath(profile.context_dir(), profile.configuration().templates_dir())
        //     };
        //
        // if !CtGen::file_exists(&canonical_templates_dir).await {
        //     return Err(CtGenError::ValidationError("Invalid templates-dir specified.".to_string()).into());
        // }
        //
        // // validate scripts dir existence and read permissions
        // let canonical_scripts_dir = if profile.configuration().scripts_dir().is_empty() || profile.configuration().scripts_dir() == "." {
        //     profile.context_dir().to_string()
        // } else {
        //     CtGen::get_filepath(profile.context_dir(), profile.configuration().scripts_dir())
        // };
        //
        // if !CtGen::file_exists(&canonical_scripts_dir).await {
        //     return Err(CtGenError::ValidationError("Invalid scripts-dir specified.".to_string()).into());
        // }
        //
        // // validate targets template existence
        // for target_name in profile.targets() {
        //     let target = profile.target(target_name).unwrap();
        //
        //     let template_canonical_path = CtGen::get_filepath(&canonical_templates_dir, format!("{}.hbs", target.template()).as_str());
        //
        //     if !CtGen::file_exists(&template_canonical_path).await {
        //         return Err(CtGenError::ValidationError(format!("Template file not found for target {}.", target_name)).into());
        //     }
        // }

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
            let table = table.cloned().unwrap();
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

        let mut context: Option<CtGenTaskContext> = None;

        if pre_create_context {
            let database = reflection_adapter.get_reflection().await?;

            context = Some(CtGenTaskContext::new(database, &table.cloned().unwrap())?);
        }

        Ok(CtGenTask {
            profile: profile.clone(),
            overrides: profile_overrides,
            prompts,
            prompt_answers: HashMap::new(),
            reflection_adapter,
            table: table.cloned(),
            context_dir: context_dir.to_string(),
            target_dir: canonical_target_dir,
            context,
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
    pub fn table(&self) -> Option<&String> {
        self.table.as_ref()
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
                self.reflection_adapter.set_database_name(answer.as_str().unwrap_or("")).await?;
            }
            CtGenTaskPrompt::PromptTable => {
                let tables = self.reflection_adapter.list_table_names().await?;
                if tables.contains(&answer.as_str().unwrap_or("").to_string()) {
                    self.table = Some(answer.as_str().unwrap_or("").to_string());
                } else {
                    return Err(CtGenError::ValidationError("Table does not exist".to_string()).into());
                }
            }
            CtGenTaskPrompt::PromptGeneric { prompt_id, prompt_data: _ } => {
                self.prompt_answers.insert(prompt_id.to_string(), answer);
            }
        }

        self.update_context().await?;

        Ok(())
    }

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
}
