use anyhow::Result;
use ctgen::{CtGen, CONFIG_NAME_DEFAULT};

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    //dotenvy::dotenv()?;

    let mut ctgen = CtGen::new().await?;

    list_profiles(&ctgen);

    if ctgen.get_profiles().len() == 0 {
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
