//! CONFIGURATION 状态：brand + feature flags + known packs + registry/tags 同步 + finish。
//!
//! 原版客户端进入 configuration 后会**主动发送**若干自发包
//! （settings/client information、custom_payload minecraft:register 等），
//! 每个等待点都必须跳过这些包直到目标包到达（v4 wait_for_* 的移植）。

use qexed_packet::Packet;
use qexed_protocol::to_client;
use qexed_protocol::to_server::configuration::finish_configuration::FinishConfiguration as ServerboundFinish;
use qexed_protocol::to_server::configuration::select_known_packs::SelectKnownPacks;

use crate::error::ServerError;
use crate::registry_sync as sync;
use crate::transport::Connection;

pub const SERVER_BRAND: &str = "qexed";

/// 处理 CONFIGURATION 状态直到 finish。
/// registry 数据按客户端 known-packs 协商结果决定是否携带完整内容。
pub async fn handle_configuration<R, W>(conn: &mut Connection<R, W>) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1. brand
    conn.send(&to_client::configuration::custom_payload::CustomPayload {
        channel: "minecraft:brand".to_string(),
        data: qexed_packet::net_types::RestBuffer(SERVER_BRAND.as_bytes().to_vec()),
    })
    .await?;

    // 2. feature flags（vanilla）
    conn.send(&to_client::configuration::feature_flags::FeatureFlags {
        features: vec![sync::VANILLA_FEATURE.to_string()],
    })
    .await?;

    // 3. known packs（vanilla core 一个）
    let known_packs = sync::known_packs();
    conn.send(&to_client::configuration::select_known_packs::SelectKnownPacks {
        known_packs,
    })
    .await?;

    // 客户端回 select_known_packs：若客户端已带 vanilla core 数据，则 registry 内容可省略。
    // 等待期间跳过客户端的自发包（settings / custom_payload register 等）。
    let selected = wait_for_known_packs(conn).await?;
    let include_contents = !sync::accepts_vanilla_core_pack(&selected.entries);
    log::debug!("registry data: include full contents = {include_contents}");

    // 4. registry 数据（每注册表一个 registry_data 包）
    let registry_packets = sync::load_registry_packets(include_contents)
        .map_err(|err| ServerError::RegistrySync(err.to_string()))?;
    log::debug!("sending registry packets: {}", registry_packets.len());
    for packet in registry_packets {
        conn.send(&packet).await?;
    }

    // 5. tags（单包，按注册表分组）
    let tag_packet = sync::load_tag_packet()
        .map_err(|err| ServerError::RegistrySync(err.to_string()))?;
    log::debug!("sending tags: {} registries", tag_packet.tags.len());
    conn.send(&tag_packet).await?;

    // 6. finish
    conn.send(&to_client::configuration::finish_configuration::FinishConfiguration::default())
        .await?;

    // 客户端回 finish（跳过期间的自发包）。
    wait_for_finish_configuration(conn).await?;
    Ok(())
}

/// 等待客户端的 select_known_packs，跳过其它 configuration 包
/// （原版客户端会先发 settings / custom_payload 等自发包）。
async fn wait_for_known_packs<R, W>(
    conn: &mut Connection<R, W>,
) -> Result<SelectKnownPacks, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    loop {
        let Some((packet_id, mut payload)) = conn.read_any("client known packs").await? else {
            continue;
        };
        match packet_id {
            SelectKnownPacks::ID => {
                use qexed_packet::PacketCodec as _;
                let mut packet = SelectKnownPacks::default();
                packet.deserialize(&mut qexed_packet::PacketReader::new(&mut payload))?;
                return Ok(packet);
            }
            other => {
                log::debug!("skip packet {other:#x} while waiting for known packs");
            }
        }
    }
}

/// 等待客户端的 finish_configuration，跳过其它 configuration 包。
async fn wait_for_finish_configuration<R, W>(
    conn: &mut Connection<R, W>,
) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    loop {
        let Some((packet_id, mut payload)) = conn.read_any("client finish").await? else {
            continue;
        };
        match packet_id {
            ServerboundFinish::ID => {
                use qexed_packet::PacketCodec as _;
                let mut packet = ServerboundFinish::default();
                packet.deserialize(&mut qexed_packet::PacketReader::new(&mut payload))?;
                let _ = packet;
                return Ok(());
            }
            other => {
                log::debug!("skip packet {other:#x} while waiting for finish configuration");
            }
        }
    }
}
