use qexed_config::app::qexed::server::{GameMode, Spawn, World};
use qexed_protocol::to_client::play::player_abilities::PlayerAbilities;

pub(super) fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

pub(super) fn translatable_component(
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

pub(super) fn favicon_bytes(favicon: &str) -> Option<Vec<u8>> {
    let payload = favicon.strip_prefix("data:image/png;base64,")?;
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload).ok()
}

pub(super) fn keep_alive_id() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

pub(super) fn chunk_coord(value: f64) -> i32 {
    (value.floor() as i32).div_euclid(16)
}

pub(super) fn dimension_type_holder_id(dimension_type: &str) -> i32 {
    match dimension_type {
        "minecraft:overworld" => 1,
        "minecraft:overworld_caves" => 2,
        "minecraft:the_end" => 3,
        "minecraft:the_nether" => 4,
        _ => 1,
    }
}

pub(super) fn player_ability_flags(game_mode: GameMode, allow_flight: bool) -> u8 {
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

pub(super) fn acknowledged_player_ability_flags(
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

pub(super) fn can_modify_world(
    world_config: &World,
    position: &qexed_packet::net_types::Position,
) -> bool {
    matches!(
        world_config.game_mode,
        GameMode::Survival | GameMode::Creative
    ) && !world_config.read_only
        && !is_spawn_protected(
            &world_config.spawn,
            world_config.spawn_protection_radius,
            position,
        )
}

pub(super) fn can_attempt_world_edit(world_config: &World) -> bool {
    matches!(
        world_config.game_mode,
        GameMode::Survival | GameMode::Creative
    )
}

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
