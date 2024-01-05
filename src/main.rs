use anyhow::Result;

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;

    println!("Hello, world!");

    Ok(())
}
