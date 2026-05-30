mod codec;
mod configuration;
mod context;
mod login;
mod status;

pub(crate) use codec::{decode_payload, read_packet_id};
#[cfg(test)]
use configuration::handle_configuration;
pub use context::ServerContext;

use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use tokio::net::TcpStream;

use codec::read_expected_packet;
use login::handle_login;
use status::handle_status;

const SERVER_BRAND: &str = "qexed";

pub async fn handle(stream: TcpStream, peer_addr: std::net::SocketAddr, context: ServerContext) {
    if let Err(err) = handle_inner(stream, peer_addr, context).await {
        if is_expected_disconnect_error(&err) {
            log::debug!(
                "connection ended before protocol completion: peer={peer_addr}, error={err:#}"
            );
        } else {
            log::error!("connection handling error: peer={peer_addr}, error={err:#}");
        }
    }
    log::debug!("connection closed: {peer_addr}");
}

async fn handle_inner(
    stream: TcpStream,
    peer_addr: std::net::SocketAddr,
    context: ServerContext,
) -> anyhow::Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    let mut packets = qexed_tcp_connect::PacketStream::new(reader);
    let mut sink = qexed_tcp_connect::PacketSink::new(writer);

    let handshake = read_expected_packet::<SetProtocol, _>(&mut packets).await?;
    log::debug!(
        "handshake received: peer={}, protocol={}, host={}, port={}, next_state={}",
        peer_addr,
        handshake.protocol_version.0,
        log_safe_host(&handshake.server_host),
        handshake.server_port,
        handshake.next_state.0
    );
    match handshake.next_state.0 {
        1 => handle_status(&mut packets, &mut sink, &context).await,
        2 | 3 => handle_login(handshake, &mut packets, &mut sink, &context).await,
        state => anyhow::bail!("unsupported handshake target state: {state}"),
    }
}

fn is_expected_disconnect_error(err: &anyhow::Error) -> bool {
    if let Some(err) = err.downcast_ref::<qexed_tcp_connect::PacketReadError>() {
        return packet_read_disconnect(err);
    }
    if let Some(err) = err.downcast_ref::<qexed_tcp_connect::PacketWriteError>() {
        return packet_write_disconnect(err);
    }
    if let Some(err) = err.downcast_ref::<std::io::Error>() {
        return io_disconnect(err);
    }

    let message = err.to_string();
    message.starts_with("connection closed while ")
}

fn packet_read_disconnect(err: &qexed_tcp_connect::PacketReadError) -> bool {
    match err {
        qexed_tcp_connect::PacketReadError::ConnectionClosedWithIncompletePacket => true,
        qexed_tcp_connect::PacketReadError::OtherError(err) => io_disconnect(err),
        qexed_tcp_connect::PacketReadError::PacketReadVarIntParseError(err) => {
            packet_read_varint_disconnect(err)
        }
        _ => false,
    }
}

fn packet_read_varint_disconnect(err: &qexed_tcp_connect::PacketReadVarIntParseError) -> bool {
    match err {
        qexed_tcp_connect::PacketReadVarIntParseError::IncompleteError => false,
        qexed_tcp_connect::PacketReadVarIntParseError::ReadError(err) => io_disconnect(err),
        qexed_tcp_connect::PacketReadVarIntParseError::TooLargeError => false,
    }
}

fn packet_write_disconnect(err: &qexed_tcp_connect::PacketWriteError) -> bool {
    match err {
        qexed_tcp_connect::PacketWriteError::OtherError(err) => io_disconnect(err),
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

fn optional_text_component(text: &str) -> Option<qexed_protocol::types::TextComponent> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text_component(text))
    }
}

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

#[cfg(test)]
mod tests;
