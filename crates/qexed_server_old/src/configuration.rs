//! CONFIGURATION 状态：brand + known packs + registry/标签同步 + finish。

use qexed_protocol::to_client;

use crate::error::ServerError;
use crate::transport::Connection;

pub const SERVER_BRAND: &str = "qexed";

/// 处理 CONFIGURATION 状态直到 finish。
pub async fn handle_configuration<R, W>(
    conn: &mut Connection<R, W>,
    registry_payload: &[u8],
) -> Result<(), ServerError>
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

    // 2. known packs（vanilla 一个）
    conn.send(&to_client::configuration::select_known_packs::SelectKnownPacks {
        known_packs: vec![qexed_protocol::types::KnownPacks {
            namespace: "minecraft".to_string(),
            id: "core".to_string(),
            version: "1.21.9".to_string(),
        }],
    })
    .await?;

    // 客户端回 select_known_packs（忽略内容，双方协商完成即可）
    let _ = conn.read_packet::<qexed_protocol::to_server::configuration::select_known_packs::SelectKnownPacks>("client known packs").await?;

    // 3. registry 数据（NBT，由调用方提供——通常来自 qexed_mojang_data 提取的注册表）
    if !registry_payload.is_empty() {
        conn.send(&to_client::configuration::custom_payload::CustomPayload {
            channel: "minecraft:registry_data".to_string(),
            data: qexed_packet::net_types::RestBuffer(registry_payload.to_vec()),
        })
        .await?;
    }

    // 4. finish
    conn.send(&to_client::configuration::finish_configuration::FinishConfiguration::default())
        .await?;

    // 客户端回 finish
    let _ = conn.read_packet::<qexed_protocol::to_server::configuration::finish_configuration::FinishConfiguration>("client finish").await?;
    Ok(())
}