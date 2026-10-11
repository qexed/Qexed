//! Status 阶段：响应 `PingStart`，可选回 `Ping`（延迟测量）。

use qexed_packet::Packet;
use qexed_protocol::{
    to_client,
    to_server::status::{ping::Ping as StatusPing, ping_start::PingStart},
};

use crate::codec::{decode_payload, read_expected_packet, read_packet_id, read_packet_or_timeout};
use crate::consts::STATUS_TIMEOUT;
use crate::error::ServerError;
use crate::handle::ServerContext;

pub(crate) async fn handle_status<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    read_expected_packet::<PingStart, _>(packets, STATUS_TIMEOUT).await?;

    let cfg = &context.config;

    // TODO: 玩家数 / sample 列表待玩家管理器落地后接入；当前用 0 / 空。
    let online_players = 0_i32;

    let json = serde_json::json!({
        "version": {
            "name": qexed_mojang_data::MC_VERSION,
            "protocol": qexed_mojang_data::PROTOCOL_VERSION,
        },
        "players": {
            "max": cfg.max_players,
            "online": online_players,
            "sample": [],
        },
        "description": {
            "text": cfg.motd,
        },
    });

    sink.send(to_client::status::server_info::ServerInfo {
        response: qexed_packet::net_types::JsonValue(json),
    })
    .await?;
    sink.flush().await?;

    // 可选 Ping → Pong（带超时，客户端直接关闭视为正常结束）
    match read_packet_or_timeout(packets, STATUS_TIMEOUT).await {
        Ok(Some(mut payload)) => {
            let id = read_packet_id(&mut payload)?;
            if id == StatusPing::ID {
                let ping = decode_payload::<StatusPing>(&mut payload)?;
                sink.send(to_client::status::ping::Ping { time: ping.time })
                    .await?;
                sink.flush().await?;
            }
        }
        Ok(None) => {}
        Err(err) => log::debug!("status ping read failed: {err}"),
    }

    Ok(())
}