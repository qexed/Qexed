use qexed_log::config::LogConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = LogConfig::default();
    println!("log level: {:?}", config.level);
    qexed_log::init().await?;
    println!("log ready");
    Ok(())
}