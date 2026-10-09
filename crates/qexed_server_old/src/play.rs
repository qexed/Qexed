//! PLAY 状态（最小可玩）：join -> 同步位置 -> keepalive 循环 -> 处理客户端基础包。

use qexed_protocol::to_client;
use qexed_protocol::to_server::play::keep_alive::KeepAlive as ClientKeepAlive;

use crate::error::ServerError;
use crate::login::LoginOutcome;
use crate::transport::Connection;

/// PLAY 阶段的服务器选项。
#[derive(Debug, Clone)]
pub struct PlayOptions {
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub dimension_name: String,
    pub dimension_type: i32,
    pub hashed_seed: i64,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self {
            view_distance: 8,
            simulation_distance: 8,
            dimension_name: "minecraft:overworld".to_string(),
            dimension_type: 0,
            hashed_seed: 0,
        }
    }
}

/// 处理 PLAY 状态直到客户端断开。返回正常退出的会话统计。
pub async fn handle_play<R, W>(
    conn: &mut Connection<R, W>,
    _login: &LoginOutcome,
    options: &PlayOptions,
    _on_chat: impl FnMut(&str),
) -> Result<PlaySession, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1. join game
    conn.send(&to_client::play::login::Login {
        entity_id: 0,
        is_hardcore: false,
        dimension_names: vec!["minecraft:overworld".to_string()],
        max_player: qexed_packet::net_types::VarInt(20),
        view_distance: qexed_packet::net_types::VarInt(options.view_distance),
        simulation_distance: qexed_packet::net_types::VarInt(options.simulation_distance),
        reduced_debug_info: false,
        enable_respawn_screen: true,
        do_limited_crafting: false,
        dimension_type: qexed_packet::net_types::VarInt(options.dimension_type),
        dimension_name: options.dimension_name.clone(),
        hashed_seed: options.hashed_seed,
        game_mode: 1, // creative
        previous_game_mode: -1,
        is_debug: false,
        is_flat: true,
        has_death_location: false,
        death_dimension_name: None,
        death_position: None,
        portal_cooldown: qexed_packet::net_types::VarInt(0),
        sea_level: qexed_packet::net_types::VarInt(63),
        enforces_secure_chat: false,
    })
    .await?;

    // 2. 位置同步
    conn.send(&to_client::play::position::Position::default()).await?;

    // 3. keepalive 循环 + 基础包处理
    let mut keepalive_counter = 0i64;
    let mut session = PlaySession::default();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                keepalive_counter += 1;
                conn.send(&to_client::play::keep_alive::KeepAlive {
                    keep_alive_id: keepalive_counter,
                })
                .await?;
            }
            packet = conn.reader.read_packet() => {
                let Some(mut payload) = packet? else { break };
                let id = crate::transport::read_packet_id(&mut payload)?;
                session.packets_received += 1;
                if id == 0x1c { // to_server play keep_alive
                    let _ka = crate::transport::decode_payload::<ClientKeepAlive>(&mut payload);
                }
                // 其他包：最小实现忽略（聊天/移动等由上层扩展）
            }
        }
    }

    Ok(session)
}

/// 会话统计。
#[derive(Debug, Default)]
pub struct PlaySession {
    pub packets_received: u64,
}