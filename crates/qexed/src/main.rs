mod audit;
mod auth;
mod bootstrap;
mod cluster_entities;
mod cluster_rpc;
mod cluster_shard;
mod code_of_conduct;
mod commands;
mod config;
mod connection;
mod console;
mod content_filter;
mod entities;
mod inventory;
mod l10n;
mod lan_discovery;
mod permissions;
mod placeholders;
mod play;
pub(crate) mod profiler;
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

use clap::Parser;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(runtime_worker_threads())
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        eprintln!("{err:#}");
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
    let args = qexed_config::app::qexed::qexed_args::ServerArgs::parse();
    let Some(config) = bootstrap::load(&args).await? else {
        return Ok(());
    };

    let context = connection::ServerContext::new(config).await?;
    server::run(context).await
}
