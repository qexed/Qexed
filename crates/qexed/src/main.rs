mod auth;
mod bootstrap;
mod connection;
mod lan_discovery;
mod server;
mod status;

rust_i18n::i18n!("locales");
shadow_rs::shadow!(build);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        log::error!("{err}");
    }
    Ok(())
}

async fn run() -> anyhow::Result<()> {
    let Some(config) = bootstrap::load().await? else {
        return Ok(());
    };

    let context = connection::ServerContext::new(config)?;
    server::run(context).await
}
