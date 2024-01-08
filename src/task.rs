use crate::error::CtGenError;
use crate::profile::CtGenProfile;
use crate::CtGen;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use database_reflection::adapter::mariadb_innodb::MariadbInnodbReflectionAdapter as DbReflection;
use database_reflection::adapter::reflection_adapter::{ReflectionAdapter, ReflectionAdapterUninitialized};
use tokio::join;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTask {
    profile: CtGenProfile,
    context_dir: String,
}

impl CtGenTask {
    pub async fn new(profile: &CtGenProfile, context_dir: &str) -> Result<Self> {
        let config = profile.configuration();
        let overrides = profile.overrides();

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
        let reflection_adapter = DbReflection::new(&dsn).connect().await?;

        println!("{}", reflection_adapter.get_database_name());

        // prepare prompts

        Ok(CtGenTask {
            profile: profile.clone(),
            context_dir: context_dir.to_string(),
        })
    }
}
