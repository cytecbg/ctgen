use anyhow::Result;
use clap::{Parser, Subcommand};
use console::style;
use ctgen::consts::CONFIG_NAME_DEFAULT;
use ctgen::error::CtGenError;
use ctgen::profile::CtGenProfileConfigOverrides;
use ctgen::task::prompt::CtGenTaskPrompt;
use ctgen::CtGen;
use database_reflection::adapter::reflection_adapter::ReflectionAdapter;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select, Sort};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled, Level};
use serde_json::Value;
use std::error::Error;
use std::fmt::Display;

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

                let profile = ctgen.add_profile(profile_name, &path).await?;

                print_info(format!("Added profile {}", style(profile.name()).cyan()));

                Ok(())
            }
            CommandConfig::List => {
                list_profiles(&ctgen);

                Ok(())
            }
            CommandConfig::Rm { name } => {
                ctgen.remove_profile(&name).await?;

                print_info(format!("Removed profile {}", style(name).cyan()));

                Ok(())
            },
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

            print_info(format!("Loading profile {}", style(profile_name).cyan()));

            ctgen.set_current_profile(profile_name).await?;

            let mut profile_overrides: Option<CtGenProfileConfigOverrides> = None;

            if env_file.is_some() || env_var.is_some() || dsn.is_some() || target_dir.is_some() {
                print_info("Overriding profile parameters");
                profile_overrides = Some(CtGenProfileConfigOverrides::new(env_file, env_var, dsn, target_dir));
            }

            let context_dir = CtGen::get_realpath(&CtGen::get_current_working_dir()?).await?;

            print_info("Creating ctgen task");

            let mut task = ctgen.create_task(&context_dir, table.as_deref(), profile_overrides).await?;

            // set pre-defined prompt answer
            if let Some(prompts) = prompt {
                print_info("Overriding prompt responses");
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

                print_info("Preparing prompts");

                for unanswered_prompt in unanswered_prompts {
                    match unanswered_prompt.clone() {
                        CtGenTaskPrompt::PromptDatabase => {
                            let options = Value::from(task.reflection_adapter().list_database_names().await?);

                            let answer = ask_prompt("Enter database name:", Some(&options), false).await?;

                            task.set_prompt_answer(&unanswered_prompt, answer).await?;
                        }
                        CtGenTaskPrompt::PromptTable => {
                            let options = Value::from(task.reflection_adapter().list_table_names().await?);

                            let answer = ask_prompt("Enter table name:", Some(&options), false).await?;

                            task.set_prompt_answer(&unanswered_prompt, answer).await?;
                        }
                        CtGenTaskPrompt::PromptGeneric { prompt_id: _, prompt_data } => {
                            let rendered_prompt = task.render_prompt(&prompt_data)?;

                            let mut answer = Value::from("");
                            if rendered_prompt.should_ask() {
                                answer = ask_prompt(
                                    rendered_prompt.prompt(),
                                    Some(rendered_prompt.options()),
                                    rendered_prompt.multiple(),
                                )
                                .await?;
                            }

                            task.set_prompt_answer(&unanswered_prompt, answer).await?;
                        }
                    }
                }
            }

            //println!("{}", serde_json::to_string(&task.context())?);

            // run
            print_info("Running ctgen task");
            Ok(task.run().await?)
        }
    }
}

/// Print info label
fn print_info(label: impl Display) {
    println!("{} {}", style("❯".to_string()).for_stderr().green(), label);
}

/// Print fail label
fn print_fail(label: impl Display) {
    println!("{} {}", style("?".to_string()).for_stderr().yellow(), label);
}

/// List profiles
fn list_profiles(ctgen: &CtGen) {
    if !ctgen.get_profiles().is_empty() {
        print_info("Installed profiles:");

        let total = ctgen.get_profiles().len();
        for (idx, (profile_name, profile_file)) in ctgen.get_profiles().iter().enumerate() {
            let idx_label = format!("[{}/{}]", (idx+1), total);

            let profile_name_label = if profile_name == CONFIG_NAME_DEFAULT {
                style(profile_name).cyan().bold()
            } else {
                style(profile_name).cyan()
            };

            println!("{}\t{}\t{}", style(idx_label).dim(), profile_name_label, style(profile_file).underlined());
        }
    } else {
        print_fail("No profiles found.");
    }
}

/// Ask prompt
async fn ask_prompt(prompt_text: &str, options: Option<&Value>, multiple: bool) -> Result<Value> {
    return if let Some(options) = options {
        if options.is_string() {
            //input with default suggestion

            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .default(options.as_str().unwrap().to_string())
                .report(true)
                .interact_text()
                .unwrap();

            return Ok(Value::from(input));
        } else if !options.is_object() && !options.is_array() {
            Err(CtGenError::RuntimeError("Invalid prompt options".to_string()).into())
        } else if multiple {
            //multi-select + sort?

            let multiselected = if options.is_object() {
                options
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<String>>()
            } else {
                options
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<String>>()
            };

            print_info(format!("Note: Use {} before {}.", style("SPACE").cyan(), style("ENTER").cyan()));

            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .items(&multiselected[..])
                .max_length(20)
                .report(true)
                .interact()
                .unwrap();

            let (multiselected, selections) = if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Would you like to sort this selection?")
                .wait_for_newline(true)
                .report(true)
                .interact()
                .unwrap()
            {
                let subset = multiselected
                    .iter()
                    .enumerate()
                    .filter(|(idx, _v)| selections.contains(idx))
                    .map(|(_k, v)| v.clone())
                    .collect::<Vec<String>>();

                print_info(format!("Note: Use {} before {}.", style("SPACE").cyan(), style("ENTER").cyan()));

                let subset_sort = Sort::with_theme(&ColorfulTheme::default())
                    .with_prompt("Sort the selected items:")
                    .items(&subset[..])
                    .interact()
                    .unwrap();

                (subset, subset_sort)
            } else {
                (multiselected, selections)
            };

            if options.is_object() {
                let mut results: Vec<String> = Vec::new();
                for selection in selections {
                    let value = multiselected[selection].clone();

                    let key = options
                        .as_object()
                        .unwrap()
                        .iter()
                        .find_map(|(k, v)| if v.as_str().unwrap() == value { Some(k.clone()) } else { None })
                        .unwrap_or(String::from(""));

                    results.push(key.clone());
                }

                Ok(Value::from(results))
            } else {
                let mut results: Vec<String> = Vec::new();
                for selection in selections {
                    results.push(multiselected[selection].clone());
                }

                Ok(Value::from(results))
            }
        } else if options.is_object() && options.as_object().unwrap().keys().eq(["0", "1"].iter()) {
            // confirm

            if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .wait_for_newline(true)
                .report(true)
                .interact()
                .unwrap()
            {
                Ok(Value::from("1"))
            } else {
                Ok(Value::from("0"))
            }
        } else {
            // select

            let selections = if options.is_object() {
                options
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<String>>()
            } else {
                options
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<String>>()
            };

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .max_length(20)
                .items(&selections[..])
                .report(true)
                .interact()
                .unwrap();

            if options.is_object() {
                let value = selections.get(selection).unwrap();
                let key = options
                    .as_object()
                    .unwrap()
                    .iter()
                    .find_map(|(k, v)| if v == value { Some(k.clone()) } else { None })
                    .unwrap_or(String::from(""));

                Ok(Value::from(key.clone()))
            } else {
                Ok(Value::from(selections.get(selection).unwrap().clone()))
            }
        }
    } else {
        //input

        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt_text)
            .interact_text()
            .unwrap();

        Ok(Value::from(input))
    };

    //Ok(Value::from(""))

    // println!("Prompt: {}", prompt_text);
    //
    // if let Some(options) = options {
    //     if options.is_string() {
    //         println!("Options: {}", options.as_str().unwrap());
    //     } else if options.is_array() {
    //         for option in options.as_array().unwrap() {
    //             println!("Option: {}", option);
    //         }
    //     } else if options.is_object() {
    //         for (option_key, option_val) in options.as_object().unwrap() {
    //             println!("Option: {} = {}", option_key, option_val);
    //         }
    //     }
    // }
    //
    // let mut input_lines = BufReader::new(tokio::io::stdin()).lines();
    //
    // if let Some(line) = input_lines.next_line().await? {
    //     if multiple {
    //         return Ok(Value::from(line.split(',').map(str::to_string).collect::<Vec<String>>()));
    //     } else {
    //         return Ok(Value::from(line));
    //     }
    // }
    //
    // Ok(Value::from(""))
}
