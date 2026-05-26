mod codec;
mod configuration;
mod context;
mod login;
mod status;

pub(crate) use codec::{decode_payload, read_packet_id};
pub use context::ServerContext;

use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use tokio::net::TcpStream;

use codec::read_expected_packet;
use configuration::handle_configuration;
use login::handle_login;
use status::handle_status;

const SERVER_BRAND: &str = "qexed";

pub async fn handle(stream: TcpStream, peer_addr: std::net::SocketAddr, context: ServerContext) {
    if let Err(err) = handle_inner(stream, peer_addr, context).await {
        log::error!("connection handling error: {err}");
    }
    log::info!("connection closed: {peer_addr}");
}

async fn handle_inner(
    stream: TcpStream,
    _peer_addr: std::net::SocketAddr,
    context: ServerContext,
) -> anyhow::Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    let mut packets = qexed_tcp_connect::PacketStream::new(reader);
    let mut sink = qexed_tcp_connect::PacketSink::new(writer);

    let handshake = read_expected_packet::<SetProtocol, _>(&mut packets).await?;
    match handshake.next_state.0 {
        1 => handle_status(&mut packets, &mut sink, &context).await,
        2 => handle_login(handshake, &mut packets, &mut sink, &context).await,
        state => anyhow::bail!("unsupported handshake target state: {state}"),
    }
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
