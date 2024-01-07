use clap::{Parser, Subcommand};
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
