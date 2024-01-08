use anyhow::Result;
use clap::{Parser, Subcommand};
use ctgen::consts::CONFIG_NAME_DEFAULT;
use ctgen::profile::CtGenProfileConfigOverrides;
use ctgen::CtGen;
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled, Level};
use std::error::Error;

#[derive(Parser, Debug)]
#[command(author = "Cytec BG", version, about = "Code Template Generator", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage code template config profiles
    Config {
        #[command(subcommand)]
        op: CommandConfig,
    },
    /// Run code template generator
    Run {
        #[arg(long, default_value = "default")]
        /// Config profile to use for this run
        profile: Option<String>,

        #[arg(long, conflicts_with = "dsn")]
        /// Override profile env-file directive
        env_file: Option<String>,

        #[arg(long, conflicts_with = "dsn")]
        /// Override profile env-var directive
        env_var: Option<String>,

        #[arg(long)]
        /// Override profile DSN directive
        dsn: Option<String>,

        #[arg(long)]
        /// Override profile target-dir directive
        target_dir: Option<String>,

        #[arg(long, value_parser = parse_prompt_key_val::<String, String>, number_of_values = 1)]
        /// Prompt answer override, for example --prompt "dummy=1"
        prompt: Option<Vec<(String, String)>>,

        /// Database table name to generate code templates for
        table: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CommandConfig {
    /// Add a config profile. If no name is given, template name from toml file will be used
    Add {
        #[arg(long, conflicts_with = "name")]
        /// Add config as default
        default: bool,
        #[arg(long)]
        /// Add config with specific name
        name: Option<String>,

        #[arg(default_value = ".")]
        /// Path to Ctgen.toml file
        path: String,
    },
    /// List all saved config profiles
    List,
    /// Remove a config profile
    Rm {
        /// Config profile name to remove
        name: String,
    },
}

pub fn parse_prompt_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;

    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    env_logger::init();

    //dotenvy::dotenv()?;

    let args = Args::parse();

    let mut ctgen = CtGen::new().await?;

    match args.command {
        Commands::Config { op } => match op {
            CommandConfig::Add { default, name, path } => {
                let profile_name = if let Some(n) = name.as_deref() {
                    n
                } else if default {
                    CONFIG_NAME_DEFAULT
                } else {
                    ""
                };

                ctgen.add_profile(profile_name, &path).await
            }
            CommandConfig::List => {
                list_profiles(&ctgen);

                Ok(())
            }
            CommandConfig::Rm { name } => ctgen.remove_profile(&name).await,
        },
        Commands::Run {
            profile,
            env_file,
            env_var,
            dsn,
            target_dir,
            prompt,
            table,
        } => {
            let profile_name = if let Some(p) = profile.as_deref() { p } else { CONFIG_NAME_DEFAULT };

            ctgen.set_current_profile(profile_name).await?;

            if env_file.is_some() || env_var.is_some() || dsn.is_some() || target_dir.is_some() {
                ctgen.set_current_profile_overrides(CtGenProfileConfigOverrides::new(env_file, env_var, dsn, target_dir));
            }

            if let Some(prompts) = prompt {
                for (prompt_id, prompt_answer) in prompts {
                    ctgen.set_current_profile_prompt_answer(&prompt_id, &prompt_answer);
                }
            }

            let profile = ctgen.get_current_profile().unwrap();

            let context_dir = CtGen::get_realpath(&CtGen::get_current_working_dir()?).await?;

            let task = ctgen.create_task(&context_dir).await?;

            println!("{:?}", task);

            //TODO

            Ok(())
        }
    }
}

fn list_profiles(ctgen: &CtGen) {
    if !ctgen.get_profiles().is_empty() {
        for (profile_name, profile_file) in ctgen.get_profiles().iter() {
            println!("Profile {} at {}", profile_name, profile_file);
        }
    } else {
        println!("No profiles found.");
    }
}
