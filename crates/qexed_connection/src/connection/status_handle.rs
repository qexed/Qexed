//! 状态 ping 处理（v4 connection::status 迁移）。
//!
//! 包名对齐 v6：PingStart → status_request::StatusRequest，
//! StatusPing → ping_request::PingRequest，
//! ServerInfo → status_response::StatusResponse，
//! to_client::status::ping::Ping → pong_response::PongResponse。

use qexed_packet::Packet;
use qexed_protocol::{
    to_client,
    to_server::status::{ping_request::PingRequest, status_request::StatusRequest},
};

use super::{
    ServerContext,
    codec::{decode_payload, read_expected_packet, read_packet_id},
};
use crate::error::Result;

pub(super) async fn handle_status<R, W>(
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    context: &ServerContext,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    read_expected_packet::<StatusRequest, _>(packets).await?;
    sink.send(to_client::status::status_response::StatusResponse {
        status: qexed_packet::net_types::JsonValue(crate::status::response(&context.config)),
    })
    .await?;

    if let Some(mut payload) = packets.read_packet().await? {
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == PingRequest::ID {
            let ping = decode_payload::<PingRequest>(&mut payload)?;
            sink.send(to_client::status::pong_response::PongResponse {
                time: ping.time,
            })
            .await?;
        }
    }

    Ok(())
}
