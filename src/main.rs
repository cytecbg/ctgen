use anyhow::Result;
use tokio::fs::read_to_string;
use ctgen::{CtGen, CONFIG_NAME_DEFAULT};

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    let mut ctgen = CtGen::new().await?;

    if !ctgen.get_profiles().is_empty() {
        for (profile_name, profile_file) in ctgen.get_profiles().iter() {
            println!("Profile {} at {}", profile_name, profile_file);
        }
    } else {
        println!("No profiles found.");
    }

    if let Err(e) = ctgen.set_profile(CONFIG_NAME_DEFAULT, "~/IdeaProjects/ctgen/Ctgen.toml").await {
        println!("Error: {}", e);
    }

    println!("{:?}", read_to_string("Ctgen.toml").await?.parse::<toml::Table>()?);

    //dotenvy::dotenv()?;

    Ok(())
}
