use anyhow::Result;
use clap::Parser;
#[allow(unused_imports)]
use log::{debug, error, log_enabled, info, Level};
use ctgen::cli::{Args, CommandConfig, Commands};
use ctgen::consts::CONFIG_NAME_DEFAULT;
use ctgen::CtGen;

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    env_logger::init();

    //dotenvy::dotenv()?;

    let args = Args::parse();

    let mut ctgen = CtGen::new().await?;

    match args.command {
        Commands::Config { op } => {
            match op {
                CommandConfig::Add { default: _, name, path} => {
                    let profile_name = if let Some(n) = name.as_deref() {
                        n
                    } else {
                        CONFIG_NAME_DEFAULT
                    };

                    ctgen.set_profile(profile_name, &path).await
                }
                CommandConfig::List => {
                    list_profiles(&ctgen);

                    Ok(())
                }
                CommandConfig::Rm { name} => {
                    ctgen.remove_profile(&name).await
                }
            }
        }
        Commands::Run { table } => {
            println!("run for {:?}", table);

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