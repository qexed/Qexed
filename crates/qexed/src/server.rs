use rust_i18n::t;
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

use crate::connection::ServerContext;

const CONNECTION_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub async fn run(context: ServerContext) -> anyhow::Result<()> {
    let tcp_server: TcpListener = qexed_tcp_connect::bind(&context.config.server.ip).await?;
    let server_port = tcp_server.local_addr()?.port();
    crate::lan_discovery::spawn(context.config.clone(), server_port);

    let max_connections = context.config.server.max_port_connections as usize;
    let semaphore = std::sync::Arc::new(Semaphore::new(max_connections));
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let mut connections = JoinSet::new();
    crate::console::spawn(context.clone(), shutdown_tx.clone());
    crate::console::spawn_ctrl_c_shutdown(shutdown_tx.clone());

    log::info!(
        "{}",
        t!("qexed.listening_on", ip = &context.config.server.ip)
    );
    loop {
        reap_finished_connections(&mut connections);

        let (stream, addr) = tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() {
                    continue;
                }
                if *shutdown_rx.borrow() {
                    log::info!("server shutdown requested from terminal console");
                    break;
                }
                continue;
            }
            accepted = tcp_server.accept() => accepted?,
        };
        let permit = tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    log::info!("server shutdown requested from terminal console");
                    break;
                }
                continue;
            }
            permit = semaphore.clone().acquire_owned() => permit?,
        };
        let active_count = max_connections.saturating_sub(semaphore.available_permits());

        log::debug!(
            "{}",
            t!(
                "qexed.debug.new_tcp_connection",
                local = stream.local_addr()?,
                peer = addr,
                count = active_count
            )
        );

        let context = context.clone();
        let connection_shutdown = shutdown_rx.clone();
        connections.spawn(async move {
            crate::connection::handle(stream, addr, context, connection_shutdown).await;
            drop(permit);
        });
    }

    drain_connections(&mut connections).await;
    context.world.flush_block_writes();

    Ok(())
}

fn reap_finished_connections(connections: &mut JoinSet<()>) {
    while let Some(result) = connections.try_join_next() {
        if let Err(err) = result {
            log::warn!("connection task failed: {err:#}");
        }
    }
}

async fn drain_connections(connections: &mut JoinSet<()>) {
    let drained = tokio::time::timeout(CONNECTION_SHUTDOWN_TIMEOUT, async {
        while let Some(result) = connections.join_next().await {
            if let Err(err) = result {
                log::warn!("connection task failed during shutdown: {err:#}");
            }
        }
    })
    .await;

    if drained.is_err() {
        let remaining = connections.len();
        log::warn!("aborting {remaining} connection task(s) after shutdown timeout");
        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(err) = result {
                if !err.is_cancelled() {
                    log::warn!("connection task failed after abort: {err:#}");
                }
            }
        }
    }
}
