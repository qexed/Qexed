const MULTICAST_ADDR: &str = "224.0.2.60:4445";
const MIN_INTERVAL_MS: u64 = 500;

pub fn spawn(config: std::sync::Arc<crate::config::RuntimeConfig>, server_port: u16) {
    if !config.server.lan_discovery.enable {
        return;
    }

    let message = announcement_message(&config.server.motd, server_port);
    let interval_ms = config.server.lan_discovery.interval_ms.max(MIN_INTERVAL_MS);

    tokio::spawn(async move {
        if let Err(err) = run(message, interval_ms).await {
            log::warn!("局域网发现广播停止: {err}");
        }
    });
}

async fn run(message: Vec<u8>, interval_ms: u64) -> anyhow::Result<()> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.set_multicast_loop_v4(false)?;
    socket.set_multicast_ttl_v4(1)?;
    socket.set_nonblocking(true)?;
    let socket = tokio::net::UdpSocket::from_std(socket)?;
    let target: std::net::SocketAddr = MULTICAST_ADDR.parse()?;
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(interval_ms));

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
                    "MC server based on Rust".to_string()
                ],
                25565
            ),
            b"[MOTD]Welcome to Qexed\nMC server based on Rust[/MOTD][AD]25565[/AD]"
        );
    }
}
