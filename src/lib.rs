pub mod cli;
pub mod consts;
pub mod error;
pub mod profile;

use crate::consts::*;
use crate::error::CtGenError;
use crate::profile::CtGenProfile;
use anyhow::Result;
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::MAIN_SEPARATOR;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGen {
    config_path: String,
    config_file: String,
    profiles: IndexMap<String, String>,
    current_profile: Option<CtGenProfile>,
}

impl CtGen {
    /// Init CtGen library
    pub async fn new() -> Result<Self> {
        let config_path = CtGen::get_config_dir()?;

        CtGen::init_config_dir(&config_path).await?;

        let config_file = CtGen::get_config_file(&config_path);

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
        CtGen::get_realpath(&CtGen::get_filepath(path, file)).await
    }

    /// Get full config filepath and filename
    pub fn get_config_file(path: &str) -> String {
        CtGen::get_filepath(path, CONFIG_FILE_NAME)
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
            .truncate(true)
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
                            profiles.insert(profile_name.to_string(), profile_file.as_str().unwrap_or("").to_string());
                        }
                    }
                }

                Ok(profiles)
            }
            Err(e) => Err(CtGenError::InitError(format!("Failed to load profiles: {}", e)).into()),
        }
    }

    async fn save_profiles(&self) -> Result<()> {
        let mut profiles_config = toml::map::Map::new();
        let mut profiles = toml::Table::new();
        for (profile_name, profile_file) in self.profiles.iter() {
            profiles.insert(profile_name.to_string(), toml::Value::String(profile_file.to_string()));
        }

        profiles_config.insert("profiles".to_string(), toml::Value::Table(profiles));

        let toml = toml::to_string_pretty(&profiles_config)
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to generate toml file: {}", e)))?;

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.config_file)
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to open toml file: {}", e)))?;

        file.write_all(toml.as_bytes())
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to write toml file: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to flush toml file: {}", e)))?;

        Ok(())
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

        if !CtGen::file_exists(&fullpath).await {
            return Err(CtGenError::ValidationError(format!("Profile config file not found: {}", fullpath)).into());
        }

        // validate content
        let profile = CtGenProfile::load(&fullpath, name).await?;
        profile.validate().await?;

        // if no name is given, we use the profile internal name
        let name = if name.is_empty() { profile.configuration().name() } else { name };

        // set profile
        self.profiles.insert(name.to_string(), fullpath.clone());

        // save profiles
        self.save_profiles().await
    }

    pub async fn remove_profile(&mut self, name: &str) -> Result<()> {
        if self.profiles.contains_key(name) {
            self.profiles.remove(name);
        }

        if let Some(profile) = self.current_profile.clone() {
            if profile.name() == name {
                self.current_profile = None;
            }
        }

        self.save_profiles().await
    }
}
