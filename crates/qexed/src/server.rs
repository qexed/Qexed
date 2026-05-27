use rust_i18n::t;
use tokio::net::TcpListener;

use crate::connection::ServerContext;

pub async fn run(context: ServerContext) -> anyhow::Result<()> {
    let tcp_server: TcpListener = qexed_tcp_connect::bind(&context.config.server.ip).await?;
    let server_port = tcp_server.local_addr()?.port();
    crate::lan_discovery::spawn(context.config.clone(), server_port);

    let max_connections = context.config.server.max_port_connections as usize;
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_connections));
    log::info!(
        "{}",
        t!("qexed.listening_on", ip = &context.config.server.ip)
    );
    loop {
        let (stream, addr) = tcp_server.accept().await?;
        let permit = semaphore.clone().acquire_owned().await?;
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
}
