//! TCP 服务器主循环：accept / 连接数闸门 / 优雅关停排水。
//! 迁移自 v4 crates/qexed/src/server.rs。
//!
//! 适配差异（v6）：
//! - v4 的 qexed_tcp_connect::bind 归 qexed_connection（空壳），这里直接
//!   tokio::net::TcpListener::bind；连接处理（connection::handle）同样归
//!   qexed_connection，以 ConnectionHandler trait 注入。
//! - lan_discovery 广播收敛为本模块 lan_discovery::spawn（保留 v4 报文格式）。
//! - anyhow → crate::error::ServerError；i18n 走 qexed_language::t。

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{Semaphore, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::config::{LanDiscoveryConfig, ServerConfig};
use crate::context::{ServerRuntime, ServerServices};
use crate::error::{Result, ServerError};

const CONNECTION_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const SERVICE_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// 单个连接的处理方（v4 crate::connection::handle）。
/// 异步方法返回 boxed future 以保持 dyn 兼容。
pub trait ConnectionHandler: Send + Sync + 'static {
    fn handle(
        &self,
        stream: tokio::net::TcpStream,
        addr: std::net::SocketAddr,
        shutdown: watch::Receiver<bool>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
}

/// 运行服务器主循环直到 shutdown。
pub async fn run(
    config: ServerConfig,
    runtime: Arc<dyn ServerRuntime>,
    services: Arc<ServerServices>,
    connections: Arc<dyn ConnectionHandler>,
) -> Result<()> {
    let tcp_server: TcpListener = TcpListener::bind(&config.ip).await.map_err(|source| {
        ServerError::IoContext {
            context: format!("failed to bind server listener: {}", config.ip),
            source,
        }
    })?;
    let server_port = tcp_server.local_addr()?.port();
    lan_discovery::spawn(&config, server_port);

    let max_connections = config.max_port_connections.max(1) as usize;
    let semaphore = Arc::new(Semaphore::new(max_connections));
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let mut connection_tasks = JoinSet::new();
    let global_services = crate::services::spawn_global_services(runtime.clone(), shutdown_rx.clone());
    crate::console::spawn(runtime.clone(), services, shutdown_tx.clone());
    crate::console::spawn_ctrl_c_shutdown(shutdown_tx.clone());

    log::info!(
        "{}",
        qexed_language::t("qexed.server.listening_on").replace("%{ip}", &config.ip)
    );
    loop {
        reap_finished_connections(&mut connection_tasks);

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
            permit = semaphore.clone().acquire_owned() => permit.map_err(|err| {
                ServerError::Message(format!("connection semaphore closed: {err}"))
            })?,
        };
        let active_count = max_connections.saturating_sub(semaphore.available_permits());

        log::debug!(
            "{}",
            qexed_language::t("qexed.server.new_tcp_connection")
                .replace("%{local}", &stream.local_addr().map(|a| a.to_string()).unwrap_or_default())
                .replace("%{peer}", &addr.to_string())
                .replace("%{count}", &active_count.to_string())
        );

        let connections = connections.clone();
        let connection_shutdown = shutdown_rx.clone();
        connection_tasks.spawn(async move {
            connections.handle(stream, addr, connection_shutdown).await;
            drop(permit);
        });
    }

    drain_connections(&mut connection_tasks).await;
    drain_global_services(global_services).await;
    Ok(())
}

fn reap_finished_connections(connections: &mut JoinSet<()>) {
    while let Some(result) = connections.try_join_next()
        && let Err(err) = result
    {
        log::warn!("connection task failed: {err}");
    }
}

async fn drain_connections(connections: &mut JoinSet<()>) {
    let drained = tokio::time::timeout(CONNECTION_SHUTDOWN_TIMEOUT, async {
        while let Some(result) = connections.join_next().await
            && let Err(err) = result
        {
            log::warn!("connection task failed during shutdown: {err}");
        }
    })
    .await;

    if drained.is_err() {
        let remaining = connections.len();
        log::warn!("aborting {remaining} connection task(s) after shutdown timeout");
        connections.abort_all();
        while let Some(result) = connections.join_next().await
            && let Err(err) = result
            && !err.is_cancelled()
        {
            log::warn!("connection task failed after abort: {err}");
        }
    }
}

async fn drain_global_services(mut services: JoinHandle<()>) {
    tokio::select! {
        result = &mut services => {
            if let Err(err) = result {
                log::warn!("global service task failed during shutdown: {err}");
            }
        }
        _ = tokio::time::sleep(SERVICE_SHUTDOWN_TIMEOUT) => {
            log::warn!("aborting global service task after shutdown timeout");
            services.abort();
            if let Err(err) = services.await
                && !err.is_cancelled()
            {
                log::warn!("global service task failed after abort: {err}");
            }
        }
    }
}

/// LAN 发现广播（迁移自 v4 lan_discovery.rs，Minecraft 多播格式原样保留）。
pub mod lan_discovery {
    use qexed_config::Config;
    use super::*;

    const MULTICAST_ADDR: &str = "224.0.2.60:4445";
    const MIN_INTERVAL_MS: u64 = 500;

    pub fn spawn(config: &ServerConfig, server_port: u16) {
        let lan = LanDiscoveryConfig::load_and_create_default(false)
            .unwrap_or_default();
        if !lan.enable {
            return;
        }

        let motd = if lan.motd.is_empty() {
            config.motd.clone()
        } else {
            vec![lan.motd.clone()]
        };
        let message = announcement_message(&motd, server_port);
        let interval_ms = MIN_INTERVAL_MS.max(MIN_INTERVAL_MS);

        tokio::spawn(async move {
            if let Err(err) = run(message, interval_ms).await {
                log::warn!("lan discovery stopped: {err}");
            }
        });
    }

    async fn run(message: Vec<u8>, interval_ms: u64) -> Result<()> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.set_multicast_loop_v4(false)?;
        socket.set_multicast_ttl_v4(1)?;
        socket.set_nonblocking(true)?;
        let socket = tokio::net::UdpSocket::from_std(socket)?;
        let target: std::net::SocketAddr = MULTICAST_ADDR
            .parse()
            .map_err(|_| ServerError::InvalidEndpoint(MULTICAST_ADDR.to_string()))?;
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(interval_ms));

        loop {
            interval.tick().await;
            socket.send_to(&message, target).await?;
        }
    }

    fn announcement_message(motd: &[String], port: u16) -> Vec<u8> {
        let motd = motd.join("\n");
        format!("[MOTD]{motd}[/MOTD][AD]{port}[/AD]").into_bytes()
    }

    #[cfg(test)]
    mod tests {
        use super::announcement_message;

        #[test]
        fn builds_minecraft_lan_discovery_message() {
            assert_eq!(
                announcement_message(
                    &[
                        "Welcome to Qexed".to_string(),
                        "MC server based on Rust".to_string(),
                    ],
                    25565,
                ),
                b"[MOTD]Welcome to Qexed\nMC server based on Rust[/MOTD][AD]25565[/AD]"
            );
        }
    }
}