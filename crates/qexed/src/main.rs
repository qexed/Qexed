mod audit;
mod auth;
mod bootstrap;
mod code_of_conduct;
mod commands;
mod config;
mod connection;
mod console;
mod content_filter;
mod entities;
mod inventory;
mod lan_discovery;
mod permissions;
mod placeholders;
mod play;
mod player_data;
mod players;
mod plugins;
mod proxy_forwarding;
mod registry_sync;
mod resource_pack;
mod secure_chat;
mod server;
mod services;
mod status;
mod structures;
mod warden;
mod world;

rust_i18n::i18n!("locales");
shadow_rs::shadow!(build);

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(runtime_worker_threads())
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        log::error!("{err}");
    }
    Ok(())
}

fn runtime_worker_threads() -> usize {
    std::env::var("QEXED_WORKER_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(2)
}

async fn run() -> anyhow::Result<()> {
    let Some(config) = bootstrap::load().await? else {
        return Ok(());
    };

    let context = connection::ServerContext::new(config).await?;
    server::run(context).await
}
