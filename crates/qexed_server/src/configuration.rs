//! Configuration 阶段：brand / feature flags / known packs / registry / tags，
//! 期间维持 KeepAlive 心跳（15s 发、30s 未回应判超时）。

use qexed_packet::Packet;
use tokio::time::{Instant, MissedTickBehavior};

use qexed_protocol::{
    to_client,
    to_server::configuration::{
        finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
        keep_alive::KeepAlive as ServerboundKeepAlive,
        select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
        settings::Settings as ServerboundSettings,
    },
};

use crate::codec::{decode_payload, read_expected_packet, read_packet_id, string_payload};
use crate::consts::{
    CONFIG_KEEPALIVE_INTERVAL, CONFIG_KEEPALIVE_TIMEOUT, CONFIG_READ_TIMEOUT,
    DEFAULT_DISPLAYED_SKIN_PARTS, SERVER_BRAND,
};
use crate::error::ServerError;

#[derive(Debug, Clone, Default)]
pub(crate) struct ClientConfiguration {
    pub(crate) locale: Option<String>,
    pub(crate) displayed_skin_parts: u8,
}

pub(crate) async fn handle_configuration<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
) -> Result<ClientConfiguration, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1. 品牌
    sink.send(to_client::configuration::custom_payload::CustomPayload {
        channel: "minecraft:brand".to_string(),
        data: qexed_packet::net_types::RestBuffer(string_payload(SERVER_BRAND)?),
    })
    .await?;

    // 2. 功能开关
    sink.send(to_client::configuration::feature_flags::FeatureFlags {
        features: vec![qexed_registry::VANILLA_FEATURE.to_string()],
    })
    .await?;

    // 3. 请求客户端选择 known packs
    let known_packs = qexed_registry::known_packs();
    sink.send(to_client::configuration::select_known_packs::SelectKnownPacks {
        known_packs: known_packs.clone(),
    })
    .await?;
    sink.flush().await?;

    // 4. 等客户端 SelectKnownPacks（期间保持 KeepAlive 心跳 + 读超时）
    let mut client = ClientConfiguration {
        displayed_skin_parts: DEFAULT_DISPLAYED_SKIN_PARTS,
        ..Default::default()
    };
    let selected = wait_for_known_packs(packets, sink, &mut client).await?;

    // 5. 客户端没选中 vanilla core → 发完整注册表内容
    let include_full = !qexed_registry::accepts_vanilla_core_pack(&selected.entries);
    log::debug!(
        "client known packs: {:?}, include_full_registry={}",
        selected.entries,
        include_full,
    );

    // 6. Registry Data（发送阶段客户端在持续读，不会空闲挂起）
    for packet in qexed_registry::load_registry_packets(include_full)
        .map_err(|err| ServerError::Protocol(format!("registry load failed: {err}")))?
    {
        sink.send(packet).await?;
    }

    // 7. Tags
    sink.send(
        qexed_registry::load_tag_packet()
            .map_err(|err| ServerError::Protocol(format!("tag load failed: {err}")))?,
    )
    .await?;

    // 8. FinishConfiguration
    sink.send(to_client::configuration::finish_configuration::FinishConfiguration {})
        .await?;
    sink.flush().await?;

    // 9. 等客户端 ack（带超时）
    read_expected_packet::<ServerboundFinishConfiguration, _>(packets, CONFIG_READ_TIMEOUT)
        .await?;

    Ok(client)
}

/// Configuration 阶段等待客户端 SelectKnownPacks 的循环：
/// - 每 15s 主动发一次 KeepAlive，30s 未收到回应判超时断开；
/// - 同循环里兼容 Settings（缓存）与 KeepAlive ack（比对 pending）；
/// - 一旦拿到 SelectKnownPacks 就返回。
async fn wait_for_known_packs<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    client: &mut ClientConfiguration,
) -> Result<ServerboundSelectKnownPacks, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut keepalive_interval = tokio::time::interval(CONFIG_KEEPALIVE_INTERVAL);
    keepalive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // interval 首次 tick 会立即触发，先消费掉，让首个 tick 发生在 INTERVAL 之后
    keepalive_interval.tick().await;

    let mut next_keepalive_id: i64 = 0;
    let mut pending_keepalive: Option<(i64, Instant)> = None;

    loop {
        // 心跳超时判定（放在循环顶，确保任意分支返回后都会重新检查）
        if let Some((_, sent_at)) = pending_keepalive {
            if sent_at.elapsed() >= CONFIG_KEEPALIVE_TIMEOUT {
                return Err(ServerError::Protocol(
                    "configuration keepalive timeout".into(),
                ));
            }
        }

        tokio::select! {
            // 定时发 KeepAlive
            _ = keepalive_interval.tick() => {
                if pending_keepalive.is_none() {
                    let id = next_keepalive_id;
                    next_keepalive_id = next_keepalive_id.wrapping_add(1);
                    sink.send(to_client::configuration::keep_alive::KeepAlive { keep_alive_id: id })
                        .await?;
                    sink.flush().await?;
                    pending_keepalive = Some((id, Instant::now()));
                    log::trace!("sent configuration keepalive id={id}");
                }
                // 若已有 pending：跳过本次发送，等待回包或超时判定
            }

            // 收包
            maybe = packets.read_packet() => {
                let Some(mut payload) = maybe? else {
                    return Err(ServerError::Protocol(
                        "connection closed while waiting for known packs".into(),
                    ));
                };
                let id = read_packet_id(&mut payload)?;

                match id {
                    ServerboundSettings::ID => {
                        let settings = decode_payload::<ServerboundSettings>(&mut payload)?;
                        client.locale = Some(settings.locale);
                        client.displayed_skin_parts = settings.displayed_skin_parts;
                    }

                    ServerboundKeepAlive::ID => {
                        let ack = decode_payload::<ServerboundKeepAlive>(&mut payload)?;
                        match pending_keepalive {
                            Some((pending_id, _)) if pending_id == ack.keep_alive_id => {
                                pending_keepalive = None;
                                log::trace!(
                                    "configuration keepalive acked id={}",
                                    ack.keep_alive_id
                                );
                            }
                            Some((pending_id, _)) => {
                                log::debug!(
                                    "unexpected keepalive ack: expected {}, got {}",
                                    pending_id,
                                    ack.keep_alive_id
                                );
                            }
                            None => {
                                log::debug!(
                                    "keepalive ack without pending: id={}",
                                    ack.keep_alive_id
                                );
                            }
                        }
                    }

                    ServerboundSelectKnownPacks::ID => {
                        return decode_payload::<ServerboundSelectKnownPacks>(&mut payload);
                    }

                    other => {
                        log::debug!("skip config packet while waiting for known packs: {other}");
                    }
                }
            }
        }
    }
}