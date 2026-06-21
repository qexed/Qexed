use std::{io::Write, sync::Arc};

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
    write_startup_log(&config, max_connections);

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

fn write_startup_log(config: &crate::bootstrap::RuntimeConfig, max_connections: usize) {
    if let Err(err) = write_startup_log_inner(config, max_connections) {
        eprintln!("failed to write qexed startup log: {err}");
    }
}

fn write_startup_log_inner(
    config: &crate::bootstrap::RuntimeConfig,
    max_connections: usize,
) -> std::io::Result<()> {
    std::fs::create_dir_all("logs")?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/qexed.log")?;
    writeln!(
        file,
        "qexed server listening on {}, max_connections={max_connections}",
        config.qexed.server.bind
    )?;
    writeln!(
        file,
        "qexed save root initialized at {}",
        config.save.root().path().display()
    )?;
    Ok(())
}
