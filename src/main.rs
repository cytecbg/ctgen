use anyhow::Result;
use clap::Parser;
use ctgen::cli::{Args, CommandConfig, Commands};
use ctgen::consts::CONFIG_NAME_DEFAULT;
use ctgen::profile::CtGenProfileConfigOverrides;
use ctgen::CtGen;
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled, Level};

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

            println!("run {} for {:?}", profile_name, table);
            println!("prompts: {:?}", prompt);

            ctgen.set_current_profile(profile_name).await?;

            if env_file.is_some() || env_var.is_some() || dsn.is_some() || target_dir.is_some() {
                ctgen.set_current_profile_overrides(CtGenProfileConfigOverrides::new(env_file, env_var, dsn, target_dir));
            }

            let profile = ctgen.get_current_profile().unwrap();

            println!("using profile {}", profile.configuration().name());

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
