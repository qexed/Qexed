use rust_i18n::t;
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, watch};

use crate::connection::ServerContext;

pub async fn run(context: ServerContext) -> anyhow::Result<()> {
    let tcp_server: TcpListener = qexed_tcp_connect::bind(&context.config.server.ip).await?;
    let server_port = tcp_server.local_addr()?.port();
    crate::lan_discovery::spawn(context.config.clone(), server_port);

    let max_connections = context.config.server.max_port_connections as usize;
    let semaphore = std::sync::Arc::new(Semaphore::new(max_connections));
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    crate::console::spawn(context.clone(), shutdown_tx.clone());
    crate::console::spawn_ctrl_c_shutdown(shutdown_tx.clone());

    log::info!(
        "{}",
        t!("qexed.listening_on", ip = &context.config.server.ip)
    );
    loop {
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
        tokio::spawn(async move {
            crate::connection::handle(stream, addr, context).await;
            drop(permit);
        });
    }

    Ok(())
}
