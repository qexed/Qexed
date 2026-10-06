//! 连接握手子模块（v4 connection/ 迁移）。
//!
//! ServerContext 里的 world/players/entities 等运行时管理器在 v6 属于其他 crate，
//! 这里收敛为连接域自身需要的状态（认证器 / 配置 / 行为守则文本 / 资源包下载服务），
//! 跨域能力（play 启动 / 封禁查询 / 插件初始化）以回调注入，避免依赖环。

mod codec;
mod configuration;
mod context;
mod login;
mod resource_pack;
mod status_handle;


#[cfg(test)]
#[cfg(test)]
use configuration::handle_configuration;
pub use context::{PlayLauncherFn, PluginsInitFn, ResourcePackOfferFn, ServerContext};
pub use login::LoginOutcome;

use qexed_protocol::to_server::handshaking::client_intention::ClientIntention;
use tokio::net::TcpStream;

use crate::error::{ConnectionError, Result};
use codec::read_expected_packet;
use login::handle_login;
use status_handle::handle_status;

const SERVER_BRAND: &str = "qexed";

/// 处理一条 TCP 连接（v4 connection::handle 同名迁移）。
pub async fn handle(
    stream: TcpStream,
    peer_addr: std::net::SocketAddr,
    context: ServerContext,
    shutdown: tokio::sync::watch::Receiver<bool>,
) {
    if let Err(err) = handle_inner(stream, peer_addr, context, shutdown).await {
        if is_expected_disconnect_error(&err) {
            log::debug!(
                "{}",
                qexed_language::t("qexed.connection.disconnected_before_completion")
                    .replace("%{peer}", &peer_addr.to_string())
                    .replace("%{error}", &err.to_string()),
            );
        } else {
            log::error!(
                "{}",
                qexed_language::t("qexed.connection.handle_error")
                    .replace("%{peer}", &peer_addr.to_string())
                    .replace("%{error}", &err.to_string()),
            );
        }
    }
    log::debug!("{}", qexed_language::t("qexed.connection.closed").replace("%{peer}", &peer_addr.to_string()));
}

async fn handle_inner(
    stream: TcpStream,
    peer_addr: std::net::SocketAddr,
    context: ServerContext,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    let mut packets = crate::transport::PacketStream::new(reader);
    let mut sink = crate::transport::PacketSink::new(writer);

    let handshake = read_expected_packet::<ClientIntention, _>(&mut packets).await?;
    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.handshake_received")
            .replace("%{peer}", &peer_addr.to_string())
            .replace("%{protocol}", &handshake.protocol_version.0.to_string())
            .replace("%{host}", &log_safe_host(&handshake.host_name))
            .replace("%{port}", &handshake.port.to_string())
            .replace("%{next_state}", &handshake.intention.0.to_string()),
    );
    // 线上协议（v4 实测同款）：next_state/intention 1=STATUS, 2=LOGIN, 3=TRANSFER。
    // 26.3 客户端 ping 服务器列表发 1，直接登录发 2。
    match handshake.intention.0 {
        1 => handle_status(&mut packets, &mut sink, &context).await,
        2 | 3 => handle_login(handshake, packets, sink, &context, shutdown).await,
        state => Err(ConnectionError::msg(format!(
            "unsupported handshake target state: {state}"
        ))),
    }
}

fn is_expected_disconnect_error(err: &ConnectionError) -> bool {
    if let ConnectionError::Io(err) = err {
        return io_disconnect(err);
    }

    match err {
        ConnectionError::Transport(crate::transport::TransportError::Read(
            crate::transport::PacketReadError::ConnectionClosedWithIncompletePacket,
        )) => true,
        ConnectionError::Transport(crate::transport::TransportError::Read(
            crate::transport::PacketReadError::OtherError { err: io_err },
        )) => io_disconnect(io_err),
        ConnectionError::Transport(crate::transport::TransportError::Write(
            crate::transport::PacketWriteError::OtherError { err: io_err },
        )) => io_disconnect(io_err),
        _ => false,
    }
}

fn io_disconnect(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::NotConnected
    ) || matches!(err.raw_os_error(), Some(10053 | 10054 | 10058))
}

fn log_safe_host(host: &str) -> String {
    const MAX_LOG_HOST_LEN: usize = 128;

    let mut host = host.split('\0').next().unwrap_or(host).to_string();
    if host.len() > MAX_LOG_HOST_LEN {
        host.truncate(MAX_LOG_HOST_LEN);
        host.push_str("...");
    }
    host
}

pub(crate) fn optional_text_component(text: &str) -> Option<qexed_protocol::types::TextComponent> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text_component(text))
    }
}

pub(crate) fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

#[cfg(test)]
mod tests;
