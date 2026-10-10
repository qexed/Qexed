//! TCP 监听与连接状态机装配。

use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;

use crate::configuration;
use crate::error::ServerError;
use crate::login::{self, ProtocolInfo};
use crate::play::{self, PlayOptions};
use crate::transport::Connection;

/// 服务器配置。
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind: String,
    pub protocol_info: ProtocolInfo,
    pub compression_threshold: i32,
    pub play: PlayOptions,
    pub motd: String,
    pub max_players: i32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:25565".to_string(),
            protocol_info: ProtocolInfo {
                protocol_version: 775,
                mc_version: "26.1.2".to_string(),
            },
            compression_threshold: 256,
            play: PlayOptions::default(),
            motd: "A qexed server".to_string(),
            max_players: 20,
        }
    }
}

/// 运行中的服务器句柄。
#[derive(Debug)]
pub struct ServerHandle {
    pub local_addr: std::net::SocketAddr,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl ServerHandle {
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

/// 启动服务器（阻塞直到 shutdown）。
pub async fn serve(config: ServerConfig) -> Result<ServerHandle, ServerError> {
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    let local_addr = listener.local_addr()?;
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let handle = ServerHandle { local_addr, shutdown: shutdown_tx };

    // 句柄立即返回给调用方；accept 循环在后台
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => break,
                accepted = listener.accept() => {
                    let Ok((socket, peer)) = accepted else { continue };
                    let config = config.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, &config).await {
                            eprintln!("connection {peer} ended: {e}");
                        }
                    });
                }
            }
        }
    });

    Ok(handle)
}

/// 单条连接的完整状态机。
pub async fn handle_connection<S>(
    socket: S,
    config: &ServerConfig,
) -> Result<(), ServerError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (read_half, write_half) = tokio::io::split(socket);
    let mut conn = Connection::new(read_half, write_half);

    // 1. 握手
    let handshake = conn.read_packet::<SetProtocol>("handshake").await?;
    match handshake.next_state.0 {
        1 => handle_status(&mut conn, config).await,
        2 => {
            // 2. 登录
            let login_outcome = login::handle_login(
                &mut conn,
                &handshake,
                &config.protocol_info,
                config.compression_threshold,
                "login rejected",
            )
            .await?;

            // 3. 配置（registry 数据由 configuration 内部从 mojang 缓存加载）
            configuration::handle_configuration(&mut conn).await?;

            // 4. 游玩
            let _session = play::handle_play(&mut conn, &login_outcome, &config.play, |_| {}).await?;
            Ok(())
        }
        other => Err(ServerError::UnexpectedState(format!("handshake next_state {other}"))),
    }
}

/// 状态 ping（服务器列表）。
async fn handle_status<R, W>(conn: &mut Connection<R, W>, config: &ServerConfig) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use qexed_protocol::to_server::status::ping::Ping as StatusPing;
    use qexed_protocol::to_server::status::ping_start::PingStart;

    let _ = conn.read_packet::<PingStart>("status ping start").await?;

    let response = serde_json::json!({
        "version": {
            "name": config.protocol_info.mc_version,
            "protocol": config.protocol_info.protocol_version,
        },
        "players": {
            "max": config.max_players,
            "online": 0,
        },
        "description": { "text": config.motd },
    });

    conn.send(&qexed_protocol::to_client::status::server_info::ServerInfo {
        response: qexed_packet::net_types::JsonValue(response),
    })
    .await?;

    // 客户端可能随后发 ping（延迟测量），回 pong 后连接结束
    if let Ok(ping) = conn.read_packet::<StatusPing>("status ping").await {
        conn.send(&qexed_protocol::to_client::status::ping::Ping {
            time: ping.time,
        })
        .await?;
    }
    Ok(())
}