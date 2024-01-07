use anyhow::Result;
use clap::Parser;
#[allow(unused_imports)]
use log::{debug, error, log_enabled, info, Level};
use ctgen::cli::Args;
use ctgen::consts::CONFIG_NAME_DEFAULT;
use ctgen::CtGen;

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    println!("{:?}", args);

    //dotenvy::dotenv()?;

    let mut ctgen = CtGen::new().await?;

    list_profiles(&ctgen);

    if ctgen.get_profiles().is_empty() {
        register_profile_default(&mut ctgen).await;
    } else {
        remove_profile_default(&mut ctgen).await;
    }

    list_profiles(&ctgen);

    Ok(())
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

async fn register_profile_default(ctgen: &mut CtGen) {
    if let Err(e) = ctgen.set_profile(CONFIG_NAME_DEFAULT, ".").await {
        println!("Error: {}", e);
    }
}

async fn remove_profile_default(ctgen: &mut CtGen) {
    if let Err(e) = ctgen.remove_profile(CONFIG_NAME_DEFAULT).await {
        println!("Error: {}", e);
    }
}
