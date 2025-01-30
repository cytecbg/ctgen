pub mod consts;
pub mod error;
pub mod profile;
pub mod task;

use crate::consts::*;
use crate::error::CtGenError;
use crate::profile::{CtGenProfile, CtGenProfileConfigOverrides};
use crate::task::CtGenTask;
use anyhow::Result;
use indexmap::IndexMap;
use regex::Regex;
use std::env;
use std::path::MAIN_SEPARATOR;
use std::sync::LazyLock;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Default, Debug)]
pub struct CtGen {
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
            config_file,
            profiles,
            ..Default::default()
        })
    }

    /// Resolve and get path to store config files
    pub fn get_config_dir() -> Result<String> {
        let path = dirs::config_dir().ok_or_else(|| CtGenError::InitError("Failed to get config directory.".to_string()))?;

        Ok(format!(
            "{}{}{}",
            path.to_str()
                .ok_or_else(|| CtGenError::InitError(format!("Failed to parse UTF-8 path: {:?}", path)))?,
            MAIN_SEPARATOR,
            CONFIG_DIR_NAME
        ))
    }

    /// Resolve and get current working directory
    pub fn get_current_working_dir() -> Result<String> {
        Ok(env::current_dir()
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to get current working directory: {}", e)))?
            .to_str()
            .map(str::to_string)
            .ok_or_else(|| CtGenError::RuntimeError("Failed to parse UTC-8 CWD path".to_string()))?)
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

        Ok(tokio::fs::canonicalize(&path)
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to resolve path: {:?}", e)))?
            .to_str()
            .map(str::to_string)
            .ok_or_else(|| CtGenError::RuntimeError(format!("Failed to parse UTC-8 path: {:?}", path)))?)
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
        if let Ok(f) = tokio::fs::File::open(file).await {
            tokio::fs::File::metadata(&f)
                .await
                .is_ok_and(|metadata| !metadata.permissions().readonly())
        } else {
            false
        }
    }

    /// Check if file exists
    pub async fn file_exists(file: &str) -> bool {
        tokio::fs::try_exists(file).await.is_ok_and(|res| res)
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
            .map_err(|_e| CtGenError::InitError(format!("Cannot flush config file: {}", config_file)))?;

        Ok(())
    }

    /// Create all necessary directories to store profiles config
    pub async fn init_config_dir(config_path: &str) -> Result<()> {
        Ok(tokio::fs::create_dir_all(&config_path)
            .await
            .map_err(|e| CtGenError::InitError(format!("Cannot create config directory: {}", e)))?)
    }

    /// Get config validation regex
    pub fn get_name_regex() -> &'static Regex {
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(CONFIG_NAME_PATTERN)
                .unwrap_or_else(|_| panic!("Failed to compile name validation regex pattern: {}", CONFIG_NAME_PATTERN))
        });

        &RE
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
                        for (profile_name, profile_file) in config_profiles
                            .as_table()
                            .ok_or_else(|| CtGenError::ValidationError("Invalid profiles table.".to_string()))?
                            .iter()
                        {
                            profiles.insert(
                                profile_name.to_string(),
                                profile_file
                                    .as_str()
                                    .ok_or_else(|| {
                                        CtGenError::ValidationError(format!("Invalid profile file for profile `{}`.", profile_name))
                                    })?
                                    .to_string(),
                            );
                        }
                    }
                }

                Ok(profiles)
            }
            Err(e) => Err(CtGenError::InitError(format!("Failed to load profiles: {}", e)).into()),
        }
    }

    /// Persist Profiles.toml file
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

    /// Add a new profile or replace existing
    pub async fn add_profile(&mut self, name: &str, path: &str) -> Result<CtGenProfile> {
        // validate name
        let regex = CtGen::get_name_regex();

        // if name is empty we will use the profile defined name later on
        if !name.is_empty() && !regex.is_match(name) {
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
        self.save_profiles().await?;

        Ok(profile)
    }

    /// Remove configuration profile
    pub async fn remove_profile(&mut self, name: &str) -> Result<()> {
        if self.profiles.contains_key(name) {
            self.profiles.swap_remove(name);
        }

        if let Some(profile) = self.current_profile.clone() {
            if profile.name() == name {
                self.current_profile = None;
            }
        }

        self.save_profiles().await
    }

    /// Load configuration profile
    pub async fn set_current_profile(&mut self, name: &str) -> Result<&CtGenProfile> {
        if let Some(profile_path) = self.profiles.get(name) {
            let profile = CtGenProfile::load(profile_path, name).await?;
            profile.validate().await?;

            self.current_profile = Some(profile);

            self.current_profile
                .as_ref()
                .ok_or_else(|| CtGenError::ValidationError("Invalid profile. No such profile found".to_string()).into())
        } else {
            Err(CtGenError::ValidationError("Invalid profile name. No such profile found".to_string()).into())
        }
    }

    /// Get currently loaded configuration profile
    pub fn get_current_profile(&self) -> Option<&CtGenProfile> {
        self.current_profile.as_ref()
    }

    /// Initialize new configuration profile
    pub async fn init_profile(&mut self, path: &str, name: &str) -> Result<CtGenProfile> {
        // validate name
        let regex = CtGen::get_name_regex();

        let fullpath = if path == "." || path == "./" {
            // default, cwd
            CtGen::get_current_working_dir()?
        } else if regex.is_match(path) {
            // just dir name, must create CWD/dirname if not exist
            CtGen::get_filepath(&CtGen::get_current_working_dir()?, path)
        } else {
            // resolve relative path
            CtGen::get_realpath(path).await?
        };

        let profile = CtGenProfile::new(&fullpath, name);

        CtGen::init_config_dir(&fullpath).await?;
        CtGen::init_config_dir(&profile.templates_dir()).await?;
        CtGen::init_config_dir(&profile.scripts_dir()).await?;

        let toml = toml::to_string(&profile).map_err(|e| CtGenError::RuntimeError(format!("Failed to generate toml file: {}", e)))?;

        // TODO hope to one day get rid of this horrific workaround
        let toml = toml.replace(
            "\n[prompt.dummy.options]\n1 = \"Yes\"\n0 = \"No\"",
            r#"options = { 1 = "Yes", 0 = "No" }"#,
        );

        let config_file = CtGen::get_filepath(&fullpath, PROFILE_DEFAULT_FILENAME);

        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&config_file)
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to open toml file: {}", e)))?;

        file.write_all(toml.as_bytes())
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to write toml file: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| CtGenError::RuntimeError(format!("Failed to flush toml file: {}", e)))?;

        for target in profile.targets() {
            let target = profile
                .target(target)
                .ok_or_else(|| CtGenError::ValidationError(format!("Target `{}` does not exist.", target)))?;

            let template_file = CtGen::get_filepath(&profile.templates_dir(), &format!("{}.hbs", target.template()));

            let template = DUMMY_TEMPLATE;

            let mut file = tokio::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(template_file)
                .await
                .map_err(|e| CtGenError::RuntimeError(format!("Failed to open template file: {}", e)))?;

            file.write_all(template.as_bytes())
                .await
                .map_err(|e| CtGenError::RuntimeError(format!("Failed to write template file: {}", e)))?;

            file.flush()
                .await
                .map_err(|e| CtGenError::RuntimeError(format!("Failed to flush template file: {}", e)))?;
        }

        self.add_profile(name, &config_file).await
    }

    /// Create generation task
    pub async fn create_task(
        &self,
        context_dir: &str,
        table: Option<&str>,
        profile_overrides: Option<CtGenProfileConfigOverrides>,
    ) -> Result<CtGenTask> {
        let real_context_path = CtGen::get_realpath(context_dir).await?;

        if let Some(profile) = self.current_profile.as_ref() {
            return CtGenTask::new(profile, &real_context_path, table, profile_overrides).await;
        }

        Err(CtGenError::RuntimeError("No current profile".to_string()).into())
    }
}
