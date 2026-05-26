use qexed_packet::Packet;
use qexed_protocol::{
    to_client,
    to_server::status::{ping::Ping as StatusPing, ping_start::PingStart},
};

use super::{
    ServerContext,
    codec::{decode_payload, read_expected_packet, read_packet_id},
};

pub(super) async fn handle_status<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    read_expected_packet::<PingStart, _>(packets).await?;
    sink.send(to_client::status::server_info::ServerInfo {
        response: qexed_packet::net_types::JsonValue(crate::status::response(&context.config)),
    })
    .await?;

    if let Some(mut payload) = packets.read_packet().await? {
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == StatusPing::ID {
            let ping = decode_payload::<StatusPing>(&mut payload)?;
            sink.send(to_client::status::ping::Ping { time: ping.time })
                .await?;
        }
    }

    Ok(())
}
