use anyhow::Result;
use clap::{Parser, Subcommand};
use ctgen::consts::CONFIG_NAME_DEFAULT;
use ctgen::profile::CtGenProfileConfigOverrides;
use ctgen::task::prompt::CtGenTaskPrompt;
use ctgen::CtGen;
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled, Level};
use serde_json::Value;
use std::error::Error;
use std::str::FromStr;
use tokio::io::{AsyncBufReadExt, BufReader};

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

            let mut profile_overrides: Option<CtGenProfileConfigOverrides> = None;

            if env_file.is_some() || env_var.is_some() || dsn.is_some() || target_dir.is_some() {
                profile_overrides = Some(CtGenProfileConfigOverrides::new(env_file, env_var, dsn, target_dir));
            }

            let context_dir = CtGen::get_realpath(&CtGen::get_current_working_dir()?).await?;

            let mut task = ctgen.create_task(&context_dir, table.as_ref(), profile_overrides).await?;

            // set pre-defined prompt answer
            if let Some(prompts) = prompt {
                let unanswered_prompts = task.prompts_unanswered(); // TODO clone not great

                for (answered_prompt_id, answered_prompt_answer) in prompts {
                    if let Some(unanswered_prompt) = unanswered_prompts.iter().find(|p| {
                        if let CtGenTaskPrompt::PromptGeneric { prompt_id, prompt_data: _ } = p {
                            return prompt_id == &answered_prompt_id;
                        }
                        false
                    }) {
                        // TODO unless prompts_unanswered is a cloned set we wouldn't be able to call mutable method
                        task.set_prompt_answer(unanswered_prompt, Value::from(answered_prompt_answer))
                            .await?;
                    }
                }
            }

            // ask prompts to prepare context
            loop {
                let unanswered_prompts = task.prompts_unanswered(); // TODO clone not great

                if unanswered_prompts.is_empty() {
                    break;
                }

                for unanswered_prompt in unanswered_prompts {
                    match unanswered_prompt.clone() {
                        CtGenTaskPrompt::PromptDatabase => {
                            let answer = ask_prompt("Enter database name:", None).await?;

                            task.set_prompt_answer(&unanswered_prompt, answer).await?;
                        }
                        CtGenTaskPrompt::PromptTable => {
                            let answer = ask_prompt("Enter table name:", None).await?;

                            task.set_prompt_answer(&unanswered_prompt, answer).await?;
                        }
                        CtGenTaskPrompt::PromptGeneric { prompt_id: _, prompt_data } => {
                            // if condition property is set, evaluate it to decide whether to proceed with the prompt
                            let mut should_ask_prompt = false;
                            if let Some(condition) = prompt_data.condition() {
                                if let Ok(condition_eval) = task.render(condition) {
                                    if condition_eval.trim() == "1" {
                                        should_ask_prompt = true;
                                    }
                                }
                            } else {
                                should_ask_prompt = true;
                            }

                            let mut answer = Value::from("0");
                            if should_ask_prompt {
                                let prompt_text = task.render(prompt_data.prompt())?;

                                answer = ask_prompt(
                                    &prompt_text,
                                    Some(&Value::from_str(&serde_json::to_string(prompt_data.options())?)?),
                                )
                                .await?;
                            }

                            task.set_prompt_answer(&unanswered_prompt, answer).await?;
                        }
                    }
                }
            }

            // run
            Ok(task.run().await?)
        }
    }
}

/// List profiles TODO make pretty
fn list_profiles(ctgen: &CtGen) {
    if !ctgen.get_profiles().is_empty() {
        for (profile_name, profile_file) in ctgen.get_profiles().iter() {
            println!("Profile {} at {}", profile_name, profile_file);
        }
    } else {
        println!("No profiles found.");
    }
}

/// Ask prompt TODO make pretty
async fn ask_prompt(prompt_text: &str, options: Option<&Value>) -> Result<Value> {
    println!("Prompt: {}", prompt_text);

    if let Some(options) = options {
        if options.is_string() {
            println!("Options: {}", options.as_str().unwrap());
        } else if options.is_array() {
            for option in options.as_array().unwrap() {
                println!("Option: {}", option);
            }
        } else if options.is_object() {
            for (option_key, option_val) in options.as_object().unwrap() {
                println!("Option: {} = {}", option_key, option_val);
            }
        }
    }

    let mut input_lines = BufReader::new(tokio::io::stdin()).lines();

    if let Some(line) = input_lines.next_line().await? {
        return Ok(Value::from(line));
    }

    Ok(Value::from(""))
}
