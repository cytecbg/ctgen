use std::collections::HashMap;
use std::slice::Iter;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::error::CtGenError;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfile {
    #[serde(default)]
    name: String,
    profile: CtGenProfileConfig,
    prompt: HashMap<String, CtGenPrompt>,
    target: HashMap<String, CtGenTarget>,
}

impl CtGenProfile {
    pub async fn load(file: &str, name: &str) -> Result<Self> {
        match tokio::fs::read_to_string(file).await {
            Ok(c) => {
                let mut profile: CtGenProfile =
                    toml::from_str(&c).map_err(|e| CtGenError::RuntimeError(format!("Failed to parse profile config: {}", e)))?;
                profile.set_name(name);

                Ok(profile)
            }
            Err(e) => Err(CtGenError::RuntimeError(format!("Failed to load profile config: {}", e)).into()),
        }
    }

    pub async fn validate(&self) -> Result<()> {
        Ok(())
    }

    pub fn set_name(&mut self, name: &str) -> &mut Self {
        self.name = name.to_string();

        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn configuration(&self) -> &CtGenProfileConfig {
        &self.profile
    }

    pub fn prompts(&self) -> Iter<'_, String> {
        self.profile.prompts.iter()
    }

    pub fn prompt(&self, prompt: &str) -> Option<&CtGenPrompt> {
        self.prompt.get(prompt)
    }

    pub fn targets(&self) -> Iter<'_, String> {
        self.profile.targets.iter()
    }

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CtGenPrompt {
    condition: Option<String>,
    prompt: String,
    #[serde(default = "CtGenPrompt::default_options")]
    options: toml::Value,
}

impl CtGenPrompt {
    pub fn default_options() -> toml::Value {
        toml::Value::Boolean(false)
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
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTarget {
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
}

