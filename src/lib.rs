use std::collections::HashMap;
use anyhow::Result;
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::{Display, Formatter};
use std::path::MAIN_SEPARATOR;
use std::slice::Iter;
use tokio::io::AsyncWriteExt;
use toml::Value;

pub const CONFIG_DIR_NAME: &str = "ctgen";
pub const CONFIG_FILE_NAME: &str = "Profiles.toml";
pub const CONFIG_NAME_DEFAULT: &str = "default";
pub const CONFIG_NAME_PATTERN: &str = r"^[a-zA-Z-_]+$";

pub const PROFILE_DEFAULT_FILENAME: &str = "Ctgen.toml";

#[derive(Clone, Debug, PartialEq)]
pub enum CtGenError {
    InitError(String),
    ValidationError(String),
    RuntimeError(String),
}

impl Display for CtGenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CtGenError::InitError(s) => {
                write!(f, "InitError: {}", s)
            }
            CtGenError::ValidationError(s) => {
                write!(f, "ValidationError: {}", s)
            }
            CtGenError::RuntimeError(s) => {
                write!(f, "RuntimeError: {}", s)
            }
        }
    }
}

impl std::error::Error for CtGenError {}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGen {
    config_path: String,
    config_file: String,
    profiles: IndexMap<String, String>,
    current_profile: Option<CtGenProfile>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenProfile {
    #[serde(default)]
    name: String,
    profile: CtGenProfileConfig,
    prompt: HashMap<String, CtGenPrompt>,
    target: HashMap<String, CtGenTarget>
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CtGenPrompt {
    condition: Option<String>,
    prompt: String,
    #[serde(default = "CtGenPrompt::default_options")]
    options: toml::Value,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTarget {
    template: String,
    target: String,
    formatter: Option<String>,
}

impl CtGen {
    /// Init CtGen library
    pub async fn new() -> Result<Self> {
        let config_path = CtGen::get_config_dir()?;

        CtGen::init_config_dir(&config_path).await?;

        let config_file = CtGen::get_config_file(&config_path).await?;

        if !CtGen::file_is_writable(&config_file).await {
            return Err(CtGenError::InitError(format!("Config file not accessible: {}", &config_file)).into());
        }

        if !CtGen::file_exists(&config_file).await {
            CtGen::init_config_file(&config_file).await?;
        }

        let profiles = CtGen::load_profiles(&config_file).await?;

        Ok(Self {
            config_path,
            config_file,
            profiles,
            ..Default::default()
        })
    }

    /// Resole and get path to store config files
    pub fn get_config_dir() -> Result<String> {
        let path = dirs::config_dir().ok_or(CtGenError::InitError("Failed to get config directory.".to_string()))?;

        Ok(format!(
            "{}{}{}",
            path.into_os_string()
                .into_string()
                .map_err(|e| CtGenError::InitError(format!("Failed to parse UTF-8 path: {:?}", e)))?,
            MAIN_SEPARATOR,
            CONFIG_DIR_NAME
        ))
    }

    pub fn get_current_working_dir() -> Result<String> {
        Ok(env::current_dir()
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to get current working directory: {}", e)))?
            .into_os_string()
            .into_string()
            .map_err(|s| CtGenError::RuntimeError(format!("Failed to parse UTC-8 path: {:?}", s)))?)
    }

    /// Get full filepath and filename
    pub fn get_filepath(path: &str, file: &str) -> String {
        format!("{}{}{}", path, MAIN_SEPARATOR, file)
    }

    /// Get canonical path
    pub async fn get_realpath(path: &str) -> Result<String> {
        let mut path = path.to_string();

        if path.starts_with('~') {
            if let Ok(home) = env::var("HOME") {
                path.remove(0);
                path.insert(0, MAIN_SEPARATOR);
                path.insert_str(0, &home);
            }
        }

        Ok(tokio::fs::canonicalize(path)
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to resolve path: {:?}", e)))?
            .into_os_string()
            .into_string()
            .map_err(|s| CtGenError::RuntimeError(format!("Failed to parse UTC-8 path: {:?}", s)))?)
    }

    /// Get full canonical filepath and filename
    pub async fn get_real_filepath(path: &str, file: &str) -> Result<String> {
        Ok(CtGen::get_realpath(&CtGen::get_filepath(path, file)).await?)
    }

    /// Get full config filepath and filename
    pub async fn get_config_file(path: &str) -> Result<String> {
        Ok(CtGen::get_real_filepath(path, CONFIG_FILE_NAME).await?)
    }

    /// Check if a given file location is writeable
    pub async fn file_is_writable(file: &str) -> bool {
        tokio::fs::try_exists(file).await.is_ok()
    }

    /// Check if file exists
    pub async fn file_exists(file: &str) -> bool {
        if let Ok(r) = tokio::fs::try_exists(file).await {
            return r;
        }

        false
    }

    /// Create an empty config file to store profiles
    async fn init_config_file(config_file: &str) -> Result<()> {
        // try to create
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config_file)
            .await
            .map_err(|e| CtGenError::InitError(format!("Cannot create config file: {}", e)))?;

        file.write("[profiles]".as_bytes())
            .await
            .map_err(|_e| CtGenError::InitError(format!("Cannot write to config file: {}", config_file)))?;

        file.flush()
            .await
            .map_err(|_e| CtGenError::InitError(format!("Cannot flush to config file: {}", config_file)))?;

        Ok(())
    }

    /// Create all necessary directories to store profiles config
    async fn init_config_dir(config_path: &str) -> Result<()> {
        Ok(tokio::fs::create_dir_all(&config_path)
            .await
            .map_err(|e| CtGenError::InitError(format!("Cannot create config directory: {}", e)))?)
    }

    /// Load profiles config file
    async fn load_profiles(config_file: &str) -> Result<IndexMap<String, String>> {
        match tokio::fs::read_to_string(config_file).await {
            Ok(c) => {
                let mut profiles: IndexMap<String, String> = IndexMap::new();

                let config = c
                    .parse::<toml::Table>()
                    .map_err(|e| CtGenError::InitError(format!("Failed to parse profiles: {}", e)))?;

                if let Some(config_profiles) = config.get("profiles") {
                    if config_profiles.is_table() {
                        for (profile_name, profile_file) in config_profiles.as_table().unwrap().iter() {
                            profiles.insert(profile_name.to_string(), profile_file.to_string());
                        }
                    }
                }

                Ok(profiles)
            }
            Err(e) => Err(CtGenError::InitError(format!("Failed to load profiles: {}", e)).into()),
        }
    }

    /// Get a list of loaded profiles
    pub fn get_profiles(&self) -> &IndexMap<String, String> {
        &self.profiles
    }

    /// Set a new profile or replace existing
    pub async fn set_profile(&mut self, name: &str, path: &str) -> Result<()> {
        // validate name
        let regex =
            Regex::new(CONFIG_NAME_PATTERN).map_err(|e| CtGenError::ValidationError(format!("Failed to compile regex pattern: {}", e)))?;

        if !regex.is_match(name) {
            return Err(CtGenError::ValidationError(format!(
                "Invalid profile name: {}. Make sure it matches {}",
                name, CONFIG_NAME_PATTERN
            ))
            .into());
        }

        // validate path
        let fullpath = if path == "." || path == "./" {
            // default, cwd
            let cwd = CtGen::get_current_working_dir()?;
            CtGen::get_real_filepath(&cwd, PROFILE_DEFAULT_FILENAME).await?
        } else if !path.ends_with(".toml") {
            // path to somewhere, no file specified
            CtGen::get_real_filepath(path, PROFILE_DEFAULT_FILENAME).await?
        } else {
            // path to a .toml file
            CtGen::get_realpath(path).await?
        };

        if CtGen::file_exists(&fullpath).await {
            println!("{fullpath}");
        }

        // validate content
        let profile = CtGenProfile::load(&fullpath, name).await?;
        println!("{:?}", profile);

        // set profile

        // save profiles

        Ok(())
    }
}

impl CtGenProfile {
    pub async fn load(file: &str, name: &str) -> Result<Self> {
        match tokio::fs::read_to_string(file).await {
            Ok(c) => {
                let mut profile: CtGenProfile = toml::from_str(&c).map_err(|e| CtGenError::RuntimeError(format!("Failed to parse profile config: {}", e)))?;
                profile.set_name(name);

                Ok(profile)
            }
            Err(e) => Err(CtGenError::RuntimeError(format!("Failed to load profile config: {}", e)).into()),
        }
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

impl CtGenPrompt {
    pub fn default_options() -> Value {
        toml::Value::Boolean(false)
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_ref().map(String::as_str)
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn options(&self) -> &toml::Value {
        &self.options
    }
}

impl CtGenTarget {
    pub fn template(&self) -> &str {
        &self.template
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn formatter(&self) -> Option<&str> {
        self.formatter.as_ref().map(String::as_str)
    }
}