mod auth;
mod audit;
mod bootstrap;
mod code_of_conduct;
mod commands;
mod connection;
mod content_filter;
mod entities;
mod inventory;
mod lan_discovery;
mod permissions;
mod play;
mod player_data;
mod players;
mod plugins;
mod proxy_forwarding;
mod registry_sync;
mod resource_pack;
mod secure_chat;
mod server;
mod status;
mod structures;
mod world;

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

    let context = connection::ServerContext::new(config).await?;
    server::run(context).await
}
