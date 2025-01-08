use crate::error::CtGenError;
use crate::CtGen;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::slice::Iter;
use toml::map::Map;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfile {
    #[serde(default, skip)]
    /// The configured name of the profile
    name: String,
    /// The profile parameters
    profile: CtGenProfileConfig,
    /// List of profile prompts
    prompt: HashMap<String, CtGenPrompt>,
    /// List of profile targets
    target: HashMap<String, CtGenTarget>,

    #[serde(skip)]
    /// Canonical context dir
    context_dir: String,
}

impl CtGenProfile {
    /// Load profile from .toml file and initialize
    pub async fn load(file: &str, name: &str) -> Result<Self> {
        match tokio::fs::read_to_string(file).await {
            Ok(c) => {
                let mut profile: CtGenProfile =
                    toml::from_str(&c).map_err(|e| CtGenError::RuntimeError(format!("Failed to parse profile config: {}", e)))?;

                if !name.is_empty() {
                    profile.set_name(name);
                } else {
                    let name = profile.profile.name().to_string();
                    profile.set_name(name.as_str());
                }

                let context_dir = Path::new(file)
                    .parent()
                    .ok_or_else(|| CtGenError::RuntimeError(format!("Failed to parse dirname from path: {}", file)))?
                    .to_str()
                    .ok_or_else(|| CtGenError::RuntimeError(format!("Failed to parse UTF-8 dirname from path: {}", file)))?;

                profile.set_context_dir(context_dir);

                Ok(profile)
            }
            Err(e) => Err(CtGenError::RuntimeError(format!("Failed to load profile config: {}", e)).into()),
        }
    }

    pub fn new(path: &str, name: &str) -> CtGenProfile {
        let mut options_table = Map::new();
        options_table.insert("1".to_string(), toml::Value::String("Yes".to_string()));
        options_table.insert("0".to_string(), toml::Value::String("No".to_string()));

        let dummy_prompt = CtGenPrompt {
            condition: None,
            enumerate: None,
            prompt: "Would you like to render the dummy target?".to_string(),
            options: toml::Value::Table(options_table),
            multiple: false,
            ordered: false,
            required: false,
        };

        let mut prompts = HashMap::new();
        prompts.insert("dummy".to_string(), dummy_prompt);

        let dummy_target = CtGenTarget {
            condition: Some("{{#if (eq prompts/dummy \"1\")}}1{{/if}}".to_string()),
            template: "dummy".to_string(),
            target: "dummy.md".to_string(),
            formatter: None,
        };

        let mut targets = HashMap::new();
        targets.insert("dummy".to_string(), dummy_target);

        CtGenProfile {
            name: name.to_string(),
            profile: CtGenProfileConfig {
                name: name.to_string(),
                env_file: ".env".to_string(),
                env_var: "DATABASE_URL".to_string(),
                dsn: "".to_string(),
                target_dir: "src".to_string(),
                templates_dir: "assets/templates".to_string(),
                scripts_dir: "assets/scripts".to_string(),
                prompts: vec!["dummy".to_string()],
                targets: vec!["dummy".to_string()],
            },
            prompt: prompts,
            target: targets,
            context_dir: path.to_string(),
        }
    }

    /// Check declared paths validity
    pub async fn validate(&self) -> Result<()> {
        // validate templates dir existence and read permissions
        let canonical_templates_dir = self.templates_dir();

        if !CtGen::file_exists(&canonical_templates_dir).await {
            return Err(CtGenError::ValidationError("Invalid templates-dir specified.".to_string()).into());
        }

        // validate scripts dir existence and read permissions
        let canonical_scripts_dir = self.scripts_dir();

        if !CtGen::file_exists(&canonical_scripts_dir).await {
            return Err(CtGenError::ValidationError("Invalid scripts-dir specified.".to_string()).into());
        }

        // validate targets template existence
        for target_name in self.targets() {
            let target = self.target(target_name).ok_or_else(|| {
                CtGenError::ValidationError(format!(
                    "Invalid target `{}`. Make sure all included targets are actually declared.",
                    target_name
                ))
            })?;

            let template_canonical_path = CtGen::get_filepath(&canonical_templates_dir, format!("{}.hbs", target.template()).as_str());

            if !CtGen::file_exists(&template_canonical_path).await {
                return Err(CtGenError::ValidationError(format!("Template file not found for target {}.", target_name)).into());
            }
        }

        Ok(())
    }

    /// Set profile given name
    fn set_name(&mut self, name: &str) -> &mut Self {
        self.name = name.to_string();

        self
    }

    /// Set context directory. Used to build templates and scripts paths.
    fn set_context_dir(&mut self, context_dir: &str) -> &mut Self {
        self.context_dir = context_dir.to_string();

        self
    }

    /// Get the profile given name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Canonical context directory; The directory containing the profile configuration file.
    pub fn context_dir(&self) -> &str {
        &self.context_dir
    }

    /// Canonical templates directory
    pub fn templates_dir(&self) -> String {
        if self.configuration().templates_dir().is_empty() || self.configuration().templates_dir() == "." {
            self.context_dir().to_string()
        } else {
            CtGen::get_filepath(self.context_dir(), self.configuration().templates_dir())
        }
    }

    /// Canonical scripts directory
    pub fn scripts_dir(&self) -> String {
        if self.configuration().scripts_dir().is_empty() || self.configuration().scripts_dir() == "." {
            self.context_dir().to_string()
        } else {
            CtGen::get_filepath(self.context_dir(), self.configuration().scripts_dir())
        }
    }

    /// Profile config
    pub fn configuration(&self) -> &CtGenProfileConfig {
        &self.profile
    }

    /// Profile prompts
    pub fn prompts(&self) -> Iter<'_, String> {
        self.profile.prompts.iter()
    }

    /// Profile prompt by name
    pub fn prompt(&self, prompt: &str) -> Option<&CtGenPrompt> {
        self.prompt.get(prompt)
    }

    /// Profile targets
    pub fn targets(&self) -> Iter<'_, String> {
        self.profile.targets.iter()
    }

    /// Profile target by name
    pub fn target(&self, target: &str) -> Option<&CtGenTarget> {
        self.target.get(target)
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfileConfig {
    /// The default name of the profile
    name: String,
    #[serde(rename = "env-file")]
    /// The default .env file name to look for
    env_file: String,
    #[serde(rename = "env-var")]
    /// The default environment variable to look for
    env_var: String,
    /// Skips env files and uses this default DSN string
    dsn: String,
    #[serde(rename = "target-dir")]
    /// Target output dir relative to CWD when running tasks
    target_dir: String,
    #[serde(rename = "templates-dir")]
    /// Templates dir relative to profile config dir
    templates_dir: String,
    #[serde(rename = "scripts-dir")]
    /// Scripts dir relative to profile config dir
    scripts_dir: String,
    /// List of prompt ids to use
    prompts: Vec<String>,
    /// List of target ids to use
    targets: Vec<String>,
}

impl CtGenProfileConfig {
    /// The default name of the profile
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The default .env file name to look for
    pub fn env_file(&self) -> &str {
        &self.env_file
    }
    /// The default environment variable to look for
    pub fn env_var(&self) -> &str {
        &self.env_var
    }
    /// Skips env files and uses this default DSN string
    pub fn dsn(&self) -> &str {
        &self.dsn
    }
    /// Target output dir relative to CWD when running tasks
    pub fn target_dir(&self) -> &str {
        &self.target_dir
    }
    /// Templates dir relative to profile config dir
    pub fn templates_dir(&self) -> &str {
        &self.templates_dir
    }
    /// Scripts dir relative to profile config dir
    pub fn scripts_dir(&self) -> &str {
        &self.scripts_dir
    }
    /// List of prompt ids to use
    pub fn prompts(&self) -> &Vec<String> {
        &self.prompts
    }
    /// List of target ids to use
    pub fn targets(&self) -> &Vec<String> {
        &self.targets
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfileConfigOverrides {
    /// Override default env file name
    env_file: Option<String>,
    /// Override default env var name
    env_var: Option<String>,
    /// Override default DSN string
    dsn: Option<String>,
    /// Override default target dir
    target_dir: Option<String>,
}

impl CtGenProfileConfigOverrides {
    /// Create a new set of override parameters
    pub fn new(env_file: Option<String>, env_var: Option<String>, dsn: Option<String>, target_dir: Option<String>) -> Self {
        Self {
            env_file,
            env_var,
            dsn,
            target_dir,
        }
    }
    /// Override default env file name
    pub fn env_file(&self) -> Option<&str> {
        self.env_file.as_deref()
    }
    /// Override default env var name
    pub fn env_var(&self) -> Option<&str> {
        self.env_var.as_deref()
    }
    /// Override default DSN string
    pub fn dsn(&self) -> Option<&str> {
        self.dsn.as_deref()
    }
    /// Override default target dir
    pub fn target_dir(&self) -> Option<&str> {
        self.target_dir.as_deref()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Interactive input from the user that adds context
pub struct CtGenPrompt {
    /// Handlebars template that receives the up-to-date context. Must render to "1" to proceed
    condition: Option<String>,
    /// Handlebars template that receives the up-to-date context. Produces comma-separated list. Enumerate prompt for each of the following values
    enumerate: Option<String>,
    /// Handlebars template that receives the up-to-date context. Renders the actual text for the prompt
    prompt: String,
    #[serde(default = "CtGenPrompt::default_options")]
    /// List of options, either handlebars template that produces comma-separated list, or toml array/map
    options: toml::Value,
    #[serde(default = "CtGenPrompt::default_multiple")]
    /// Flag that controls whether we allow a single answer or multiple choice
    multiple: bool,
    #[serde(default = "CtGenPrompt::default_ordered")]
    /// Flag that controls whether we care about the order of multiple valued prompts
    ordered: bool,
    #[serde(default = "CtGenPrompt::default_required")]
    /// Flag that controls whether empty answers are allowed
    required: bool,
}

impl CtGenPrompt {
    /// Default options value
    pub fn default_options() -> toml::Value {
        toml::Value::Boolean(false)
    }
    /// Default multiple flag value
    pub fn default_multiple() -> bool {
        false
    }
    /// Default required flag value
    pub fn default_required() -> bool {
        false
    }
    /// Default ordered flag value
    pub fn default_ordered() -> bool {
        false
    }

    /// Prompt condition template. If it doesn't evaluate to "1", the prompt will be skipped
    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
    /// Prompt enumerator template. If it doesn't evaluate to a comma-separated list, prompt will be skipped
    pub fn enumerate(&self) -> Option<&str> {
        self.enumerate.as_deref()
    }
    /// Prompt text template. The template that renders the prompt text
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    /// Either a template that outputs a comma-separated list, or a toml map/array
    pub fn options(&self) -> &toml::Value {
        &self.options
    }
    /// Flag that controls the number of answers allowed
    pub fn multiple(&self) -> bool {
        self.multiple
    }
    /// Flag that controls whether we care about the order of multiple valued prompts
    pub fn ordered(&self) -> bool {
        self.ordered
    }
    /// Flag that controls empty answers
    pub fn required(&self) -> bool {
        self.required
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTarget {
    /// Handlebars template that receives the up-to-date context. Must render to "1" to proceed
    condition: Option<String>,
    /// Template name. Relative to templates dir, no file extension.
    template: String,
    /// Handlebars template that receives the up-to-date context. Output file path relative to target dir.
    target: String,
    /// Handlebars template that receives the up-to-date context. Renders an optional shell command to execute after target rendering is completed
    formatter: Option<String>,
}

impl CtGenTarget {
    /// Handlebars template that receives the up-to-date context. Must render to "1" to proceed
    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
    /// Template name. Relative to templates dir, no file extension.
    pub fn template(&self) -> &str {
        &self.template
    }
    /// Handlebars template that receives the up-to-date context. Output file path relative to target dir.
    pub fn target(&self) -> &str {
        &self.target
    }
    /// Handlebars template that receives the up-to-date context. Renders an optional shell command to execute after target rendering is completed
    pub fn formatter(&self) -> Option<&str> {
        self.formatter.as_deref()
    }
}
