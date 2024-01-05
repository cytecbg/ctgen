use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use tokio::io::AsyncWriteExt;
use std::path::MAIN_SEPARATOR;

pub const CONFIG_DIR_NAME: &str = "ctgen";
pub const CONFIG_FILE_NAME: &str = "Profiles.toml";

#[derive(Clone, Debug, PartialEq)]
pub enum CtGenError {
    InitError(String),
}

impl Display for CtGenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CtGenError::InitError(s) => {
                write!(f, "InitError: {}", s)
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
    name: String,
    default_name: String,
    env_file: String,
    env_var: String,
    database_connection: String,
    target_dir: String,
    templates_dir: String,
    scripts_dir: String,
    prompts: IndexMap<String, CtGenPrompt>,
    targets: IndexMap<String, CtGenTarget>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenPrompt {
    condition: Option<String>,
    prompt: String,
    options: toml::Table,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTarget {
    template: String,
    target: String,
    formatter: Option<String>,
}

impl CtGen {
    pub async fn new() -> Result<Self> {
        let config_path = CtGen::get_config_dir()?;

        CtGen::init_config_dir(&config_path).await?;

        let config_file = CtGen::get_config_file(&config_path);

        if !CtGen::config_file_is_writable(&config_file).await {
            return Err(CtGenError::InitError(format!(
                "Config file not accessible: {}",
                &config_file
            ))
            .into());
        }

        if !CtGen::config_file_exists(&config_file).await {
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

    pub fn get_config_dir() -> Result<String> {
        let path = dirs::config_dir().ok_or(CtGenError::InitError(
            "Failed to get config directory.".to_string(),
        ))?;

        Ok(format!(
            "{}{}{}",
            path.into_os_string()
                .into_string()
                .map_err(|e| CtGenError::InitError(format!(
                    "Failed to parse UTF-8 path: {:?}",
                    e
                )))?,
            MAIN_SEPARATOR,
            CONFIG_DIR_NAME
        ))
    }

    pub fn get_config_file(path: &str) -> String {
        format!("{}{}{}", path, MAIN_SEPARATOR, CONFIG_FILE_NAME)
    }

    pub async fn config_file_is_writable(file: &str) -> bool {
        tokio::fs::try_exists(file).await.is_ok()
    }

    pub async fn config_file_exists(file: &str) -> bool {
        if let Ok(r) = tokio::fs::try_exists(file).await {
            return r;
        }

        false
    }

    async fn init_config_file(config_file: &str) -> Result<()> {
        // try to create
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config_file)
            .await
            .map_err(|e| {
                CtGenError::InitError(format!("Cannot create config file: {}", e))
            })?;

        file.write("[profiles]".as_bytes()).await.map_err(|_e| {
            CtGenError::InitError(format!("Cannot write to config file: {}", config_file))
        })?;

        file.flush().await.map_err(|_e| {
            CtGenError::InitError(format!("Cannot flush to config file: {}", config_file))
        })?;

        Ok(())
    }

    async fn init_config_dir(config_path: &str) -> Result<()> {
        Ok(tokio::fs::create_dir_all(&config_path).await.map_err(|e| {
            CtGenError::InitError(format!("Cannot create config directory: {}", e))
        })?)
    }

    async fn load_profiles(config_file: &str) -> Result<IndexMap<String, String>> {
        match tokio::fs::read_to_string(config_file).await {
            Ok(c) => {
                let mut profiles: IndexMap<String, String> = IndexMap::new();

                let config = c.parse::<toml::Table>().map_err(|e| CtGenError::InitError(format!("Failed to parse profiles: {}", e)))?;

                if let Some(config_profiles) = config.get("profiles") {
                    if config_profiles.is_table() {
                        for (profile_name, profile_file) in config_profiles.as_table().unwrap().iter() {
                            profiles.insert(profile_name.to_string(), profile_file.to_string());
                        }
                    }
                }

                Ok(profiles)
            }
            Err(e) => {
                Err(CtGenError::InitError(format!("Failed to load profiles: {}", e)).into())
            }
        }
    }

    pub fn get_profiles(&self) -> &IndexMap<String, String> {
        &self.profiles
    }
}
