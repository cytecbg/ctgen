use crate::error::CtGenError;
use crate::CtGen;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::slice::Iter;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfile {
    #[serde(default)]
    name: String,
    profile: CtGenProfileConfig,
    prompt: HashMap<String, CtGenPrompt>,
    target: HashMap<String, CtGenTarget>,

    /// Canonical context dir
    #[serde(skip)]
    context_dir: String,
}

impl CtGenProfile {
    /// Load profile from .toml file and initialize
    pub async fn load(file: &str, name: &str) -> Result<Self> {
        match tokio::fs::read_to_string(file).await {
            Ok(c) => {
                let mut profile: CtGenProfile =
                    toml::from_str(&c).map_err(|e| CtGenError::RuntimeError(format!("Failed to parse profile config: {}", e)))?;
                profile.set_name(name);

                let context_dir = Path::new(file)
                    .parent()
                    .ok_or(CtGenError::RuntimeError(format!("Failed to parse dirname from path: {}", file)))?
                    .to_str()
                    .ok_or(CtGenError::RuntimeError(format!(
                        "Failed to parse UTF-8 dirname from path: {}",
                        file
                    )))?;

                profile.set_context_dir(context_dir);

                Ok(profile)
            }
            Err(e) => Err(CtGenError::RuntimeError(format!("Failed to load profile config: {}", e)).into()),
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
            let target = self.target(target_name).unwrap();

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
    name: String,
    #[serde(rename = "env-file")]
    env_file: String,
    #[serde(rename = "env-var")]
    env_var: String,
    dsn: String,
    #[serde(rename = "target-dir")]
    target_dir: String,
    #[serde(rename = "templates-dir")]
    templates_dir: String,
    #[serde(rename = "scripts-dir")]
    scripts_dir: String,
    prompts: Vec<String>,
    targets: Vec<String>,
}

impl CtGenProfileConfig {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn env_file(&self) -> &str {
        &self.env_file
    }
    pub fn env_var(&self) -> &str {
        &self.env_var
    }
    pub fn dsn(&self) -> &str {
        &self.dsn
    }
    pub fn target_dir(&self) -> &str {
        &self.target_dir
    }
    pub fn templates_dir(&self) -> &str {
        &self.templates_dir
    }
    pub fn scripts_dir(&self) -> &str {
        &self.scripts_dir
    }
    pub fn prompts(&self) -> &Vec<String> {
        &self.prompts
    }
    pub fn targets(&self) -> &Vec<String> {
        &self.targets
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfileConfigOverrides {
    env_file: Option<String>,
    env_var: Option<String>,
    dsn: Option<String>,
    target_dir: Option<String>,
}

impl CtGenProfileConfigOverrides {
    pub fn new(env_file: Option<String>, env_var: Option<String>, dsn: Option<String>, target_dir: Option<String>) -> Self {
        Self {
            env_file,
            env_var,
            dsn,
            target_dir,
        }
    }
    pub fn env_file(&self) -> Option<&str> {
        self.env_file.as_deref()
    }
    pub fn env_var(&self) -> Option<&str> {
        self.env_var.as_deref()
    }
    pub fn dsn(&self) -> Option<&str> {
        self.dsn.as_deref()
    }
    pub fn target_dir(&self) -> Option<&str> {
        self.target_dir.as_deref()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CtGenPrompt {
    condition: Option<String>,
    prompt: String,
    #[serde(default = "CtGenPrompt::default_options")]
    options: toml::Value,
    #[serde(default = "CtGenPrompt::default_multiple")]
    multiple: bool
}

impl CtGenPrompt {
    pub fn default_options() -> toml::Value {
        toml::Value::Boolean(false)
    }
    pub fn default_multiple() -> bool {
        false
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn options(&self) -> &toml::Value {
        &self.options
    }
    pub fn multiple(&self) -> bool { self.multiple }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTarget {
    condition: Option<String>,
    template: String,
    target: String,
    formatter: Option<String>,
}

impl CtGenTarget {

    pub fn template(&self) -> &str {
        &self.template
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn formatter(&self) -> Option<&str> {
        self.formatter.as_deref()
    }
    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
}
