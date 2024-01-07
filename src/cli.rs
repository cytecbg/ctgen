use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author = "Cytec BG", version, about = "Code Template Generator", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
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
        /// Database table name to generate code templates for
        table: Option<String>
    }
}

impl Default for Commands {
    fn default() -> Self {
        Commands::Run {
            table: None
        }
    }
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
        path: String
    },
    /// List all saved config profiles
    List,
    /// Remove a config profile
    Rm {
        /// Config profile name to remove
        name: String
    }
}