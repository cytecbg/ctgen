use anyhow::Result;
use ctgen::CtGen;

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    let ctgen = CtGen::new().await?;

    if !ctgen.get_profiles().is_empty() {
        for (profile_name, profile_file) in ctgen.get_profiles().iter() {
            println!("Profile {} at {}", profile_name, profile_file);
        }
    } else {
        println!("No profiles found.");
    }

    //dotenvy::dotenv()?;

    Ok(())
}
