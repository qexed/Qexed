//! play 会话通用辅助（v4 play/util.rs 迁移）。
//!
//! v6 适配：GameMode/Spawn/World 改用 crate::config（v4 路径不存在）；
//! EntityPosition 改用 qexed_protocol::types（v4 在 add_entity 包内）。

// play-gameplay 接线前部分 API 暂未被本 crate 引用（供其改造 runtime.rs 时使用）。
#![allow(dead_code)]


use qexed_protocol::to_client::play::player_abilities::PlayerAbilities;

use crate::config::{GameMode, Spawn, WorldConfig};

pub fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

pub fn translatable_component(
    key: impl Into<String>,
    with: Vec<qexed_protocol::types::TextComponent>,
) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(key.into())),
    );
    if !with.is_empty() {
        map.insert(
            "with".to_string(),
            qexed_nbt::Tag::List(
                qexed_nbt::ListHeader {
                    tag_id: qexed_nbt::tag_id::COMPOUND,
                    length: with.len() as i32,
                },
                std::sync::Arc::from(with.into_boxed_slice()),
            ),
        );
    }
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

/// data URI favicon 解码（base64 -> PNG 字节）。
pub fn favicon_bytes(favicon: &str) -> Option<Vec<u8>> {
    let payload = favicon.strip_prefix("data:image/png;base64,")?;
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()
}

pub fn keep_alive_id() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

pub fn chunk_coord(value: f64) -> i32 {
    (value.floor() as i32).div_euclid(16)
}

pub fn dimension_type_holder_id(dimension_type: &str) -> i32 {
    match dimension_type {
        "minecraft:overworld" => 1,
        "minecraft:overworld_caves" => 2,
        "minecraft:the_end" => 3,
        "minecraft:the_nether" => 4,
        _ => 1,
    }
}

pub fn player_ability_flags(game_mode: GameMode, allow_flight: bool) -> u8 {
    let mut flags = match game_mode {
        GameMode::Survival | GameMode::Adventure => 0,
        GameMode::Creative => PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD,
        GameMode::Spectator => {
            PlayerAbilities::INVULNERABLE | PlayerAbilities::FLYING | PlayerAbilities::CAN_FLY
        }
    };
    if allow_flight {
        flags |= PlayerAbilities::CAN_FLY;
    }
    flags
}

pub fn acknowledged_player_ability_flags(
    game_mode: GameMode,
    allow_flight: bool,
    requested_flags: u8,
) -> u8 {
    let mut flags = player_ability_flags(game_mode, allow_flight);
    if flags & PlayerAbilities::CAN_FLY != 0 && requested_flags & PlayerAbilities::FLYING != 0 {
        flags |= PlayerAbilities::FLYING;
    }
    flags
}

pub fn can_modify_world(
    world_config: &WorldConfig,
    position: &qexed_packet::net_types::Position,
) -> bool {
    can_modify_world_for_game_mode(world_config.game_mode, world_config, position)
}

pub fn can_modify_world_for_game_mode(
    game_mode: GameMode,
    world_config: &WorldConfig,
    position: &qexed_packet::net_types::Position,
) -> bool {
    matches!(game_mode, GameMode::Survival | GameMode::Creative)
        && !world_config.read_only
        && !is_spawn_protected(
            &world_config.spawn,
            world_config.spawn_protection_radius,
            position,
        )
}

pub fn can_attempt_world_edit(world_config: &WorldConfig) -> bool {
    can_attempt_world_edit_for_game_mode(world_config.game_mode, world_config)
}

pub fn can_attempt_world_edit_for_game_mode(
    game_mode: GameMode,
    _world_config: &WorldConfig,
) -> bool {
    matches!(game_mode, GameMode::Survival | GameMode::Creative)
}

/// 出生点保护半径判定（v4 is_spawn_protected）。
fn is_spawn_protected(
    spawn: &Spawn,
    radius: i32,
    position: &qexed_packet::net_types::Position,
) -> bool {
    let radius = radius.max(0);
    radius > 0
        && (position.x - spawn.x.floor() as i32).abs() <= radius
        && (position.z - spawn.z.floor() as i32).abs() <= radius
}

/// 出生点位置（v4 survival::spawn_position 的等价辅助，供会话初始化用）。
pub fn spawn_position(spawn: &Spawn) -> qexed_protocol::types::EntityPosition {
    qexed_protocol::types::EntityPosition {
        x: spawn.x,
        y: spawn.y,
        z: spawn.z,
        yaw: spawn.yaw,
        pitch: spawn.pitch,
        on_ground: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_coord_matches_floor_division() {
        assert_eq!(chunk_coord(0.0), 0);
        assert_eq!(chunk_coord(15.9), 0);
        assert_eq!(chunk_coord(16.0), 1);
        assert_eq!(chunk_coord(-0.1), -1);
        assert_eq!(chunk_coord(-16.0), -1);
        assert_eq!(chunk_coord(-16.1), -2);
    }

    #[test]
    fn ability_flags_follow_game_mode() {
        assert_eq!(player_ability_flags(GameMode::Survival, false), 0);
        assert_eq!(
            player_ability_flags(GameMode::Creative, false),
            PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD
        );
        assert_eq!(player_ability_flags(GameMode::Adventure, true), PlayerAbilities::CAN_FLY);
        let spectator = player_ability_flags(GameMode::Spectator, false);
        assert_ne!(spectator & PlayerAbilities::INVULNERABLE, 0);
    }

    #[test]
    fn spawn_protection_bounds_are_inclusive() {
        let world = WorldConfig {
            spawn_protection_radius: 2,
            ..WorldConfig::default()
        };
        assert!(can_modify_world(
            &world,
            &qexed_packet::net_types::Position { x: 3, y: 64, z: 3 },
        ));
        assert!(!can_modify_world(
            &world,
            &qexed_packet::net_types::Position { x: 2, y: 64, z: 0 },
        ));
    }
}
