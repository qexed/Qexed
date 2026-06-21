mod bootstrap;
mod connection;
mod server;
#[cfg(test)]
mod worldgen_process;

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
        tklog::error!(err);
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
    let args = qexed_config::app::qexed::args::ServerArgs::parse();
    let Some(config) = bootstrap::load(&args).await? else {
        return Ok(());
    };

    server::run(config).await
}
