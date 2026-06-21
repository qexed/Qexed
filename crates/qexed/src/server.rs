use std::sync::Arc;

use tokio::{net::TcpListener, sync::Semaphore};

pub async fn run(config: crate::bootstrap::RuntimeConfig) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&config.qexed.server.bind).await?;
    let max_connections = config.qexed.server.max_connections.max(1);
    let limiter = Arc::new(Semaphore::new(max_connections));

    tklog::info!(format!(
        "qexed server listening on {}, max_connections={max_connections}",
        config.qexed.server.bind
    ));
    tklog::info!(format!(
        "qexed save root initialized at {}",
        config.save.root().path().display()
    ));

    loop {
        let (stream, peer) = listener.accept().await?;
        let permit = match limiter.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                tklog::warn!(format!("reject connection from {peer}: server is full"));
                continue;
            }
        };
        let config = config.clone();

        tokio::spawn(async move {
            let _permit = permit;
            crate::connection::handle(stream, config).await;
        });
    }
}
