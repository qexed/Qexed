use super::{
    EntityIdAllocator, EntityManager, EntitySpawnRequest, ManagedEntity, ManagedEntityKind,
    entity_type_id, npc_profile_name,
};
use bytes::{Bytes, BytesMut};
use qexed_packet::Packet;
use qexed_protocol::{
    to_client::play::{
        add_entity::EntityPosition, player_info_update::PlayerInfoUpdate,
        set_entity_data::SetEntityData,
    },
    types::EntityMetadataEnum,
};
use std::collections::BTreeMap;

fn stone_block_state() -> i32 {
    crate::inventory::placed_block_state_for_item(&qexed_protocol::types::Slot {
        item_count: qexed_packet::net_types::VarInt(1),
        item_id: Some(qexed_packet::net_types::VarInt(1)),
        ..Default::default()
    })
    .expect("stone block state")
}

fn block_state_for_item_name(name: &str) -> i32 {
    let item_id = crate::inventory::item_id_for_name(name).expect("item id");
    crate::inventory::placed_block_state_for_item(&qexed_protocol::types::Slot {
        item_count: qexed_packet::net_types::VarInt(1),
        item_id: Some(qexed_packet::net_types::VarInt(item_id)),
        ..Default::default()
    })
    .expect("block state")
}

fn empty_world() -> crate::world::WorldManager {
    crate::world::WorldManager::new(tempfile::tempdir().expect("temp world dir").keep())
}

fn test_position(x: f64, y: f64, z: f64) -> EntityPosition {
    EntityPosition {
        x,
        y,
        z,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    }
}

fn test_manager_and_players() -> (EntityManager, crate::players::PlayerManager) {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    (manager, crate::players::PlayerManager::new(entity_ids))
}

fn join_test_player(
    players: &crate::players::PlayerManager,
    username: &str,
    position: EntityPosition,
) -> crate::players::PlayerSession {
    players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: username.to_string(),
            properties: Vec::new(),
        },
        position,
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    )
}

fn spawn_vanilla_entity(
    manager: &EntityManager,
    key: &str,
    entity_type: &str,
    position: EntityPosition,
    ai_params: BTreeMap<String, serde_json::Value>,
) {
    let name = entity_type
        .trim_start_matches("minecraft:")
        .replace('_', " ");
    manager
        .spawn_local(EntitySpawnRequest {
            key: key.to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: entity_type.to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position,
            name: name.clone(),
            display_name: name,
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params,
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();
}

fn tick_entities(
    manager: &EntityManager,
    players: &crate::players::PlayerManager,
    world: &crate::world::WorldManager,
) {
    manager
        .tick_ai(
            players,
            world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();
}

fn place_stone_floor(
    world: &crate::world::WorldManager,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    y: i32,
) {
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            world.set_runtime_block(
                "minecraft:overworld",
                qexed_packet::net_types::Position { x, y, z },
                stone_block_state(),
            );
        }
    }
}

fn drain_player_events(session: &mut crate::players::PlayerSession) {
    while session.receiver.try_recv().is_ok() {}
}

#[test]
fn entity_type_id_is_loaded_from_current_report() {
    assert_eq!(entity_type_id("minecraft:player").unwrap(), 155);
    assert_eq!(entity_type_id("minecraft:armor_stand").unwrap(), 5);
    assert_eq!(entity_type_id("minecraft:text_display").unwrap(), 131);
    assert_eq!(entity_type_id("minecraft:item").unwrap(), 71);
}

#[test]
fn configured_entities_allocate_before_players() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let config = qexed_config::app::qexed::server::Entities {
        enable: true,
        dimension: "minecraft:overworld".to_string(),
        list: vec![
            qexed_config::app::qexed::server::Entity {
                id: "spawn-guide".to_string(),
                name: "Guide".to_string(),
                kind: qexed_config::app::qexed::server::EntityKind::Npc,
                ..Default::default()
            },
            qexed_config::app::qexed::server::Entity {
                id: "marker".to_string(),
                entity_type: "minecraft:armor_stand".to_string(),
                ..Default::default()
            },
        ],
        disabled_entity_types: Vec::new(),
        ai_overrides: Vec::new(),
        spawning: Default::default(),
    };

    let manager = EntityManager::from_config(&config, entity_ids.clone()).unwrap();
    let entities = manager.list_for_dimension("minecraft:overworld");

    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].entity_id, 1);
    assert_eq!(entities[0].kind, ManagedEntityKind::Npc);
    assert_eq!(entities[1].entity_id, 2);
    assert_eq!(entity_ids.next(), 3);
}

#[test]
fn configured_npc_with_unknown_client_entity_type_falls_back_to_player() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let config = qexed_config::app::qexed::server::Entities {
        enable: true,
        dimension: "minecraft:overworld".to_string(),
        list: vec![qexed_config::app::qexed::server::Entity {
            id: "guard".to_string(),
            name: "Guard".to_string(),
            kind: qexed_config::app::qexed::server::EntityKind::Npc,
            entity_type: "demo:patrol_guard".to_string(),
            ..Default::default()
        }],
        disabled_entity_types: Vec::new(),
        ai_overrides: Vec::new(),
        spawning: Default::default(),
    };

    let manager = EntityManager::from_config(&config, entity_ids).unwrap();
    let entities = manager.list_for_dimension("minecraft:overworld");

    assert_eq!(entities[0].kind, ManagedEntityKind::Npc);
    assert_eq!(entities[0].entity_type, "minecraft:player");
    assert_eq!(
        entities[0].entity_type_id,
        entity_type_id("minecraft:player").unwrap()
    );
}

#[test]
fn configured_holograms_spawn_as_text_display_entities() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let config = qexed_config::app::qexed::server::Entities {
        enable: true,
        dimension: "minecraft:overworld".to_string(),
        list: vec![qexed_config::app::qexed::server::Entity {
            id: "welcome-title".to_string(),
            name: "Welcome".to_string(),
            kind: qexed_config::app::qexed::server::EntityKind::Hologram,
            y: 67.0,
            ..Default::default()
        }],
        disabled_entity_types: Vec::new(),
        ai_overrides: Vec::new(),
        spawning: Default::default(),
    };

    let manager = EntityManager::from_config(&config, entity_ids).unwrap();
    let entities = manager.list_for_dimension("minecraft:overworld");
    let packets = manager
        .spawn_packets_for_dimension("minecraft:overworld")
        .unwrap();

    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].kind, ManagedEntityKind::Hologram);
    assert_eq!(entities[0].entity_type, "minecraft:text_display");
    assert_eq!(entities[0].entity_type_id, 131);
    assert_eq!(packets.len(), 3);
    assert_eq!(
        packets[0][0],
        qexed_protocol::to_client::play::add_entity::AddEntity::ID as u8
    );
    assert_eq!(
        packets[2][0],
        qexed_protocol::to_client::play::set_entity_data::SetEntityData::ID as u8
    );
}

#[test]
fn configured_entities_respect_disabled_types_and_ai_overrides() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let config = qexed_config::app::qexed::server::Entities {
        enable: true,
        dimension: "minecraft:overworld".to_string(),
        list: vec![
            qexed_config::app::qexed::server::Entity {
                id: "disabled-zombie".to_string(),
                entity_type: "minecraft:zombie".to_string(),
                ..Default::default()
            },
            qexed_config::app::qexed::server::Entity {
                id: "arena-pig".to_string(),
                entity_type: "minecraft:pig".to_string(),
                ai: "random_stroll".to_string(),
                auto_jump: true,
                ..Default::default()
            },
        ],
        disabled_entity_types: vec!["zombie".to_string()],
        ai_overrides: vec![qexed_config::app::qexed::server::EntityAiOverride {
            entity_type: "minecraft:pig".to_string(),
            ai: "vanilla".to_string(),
            ai_params: [("movement_speed".to_string(), serde_json::json!(0.25))]
                .into_iter()
                .collect(),
            auto_jump: Some(false),
        }],
        spawning: Default::default(),
    };

    let manager = EntityManager::from_config(&config, entity_ids).unwrap();
    let entities = manager.list_for_dimension("minecraft:overworld");

    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].entity_type, "minecraft:pig");
    assert_eq!(entities[0].ai, "vanilla");
    assert_eq!(entities[0].ai_params["movement_speed"], 0.25);
    assert!(!entities[0].auto_jump);
}

#[test]
fn dropped_items_are_collected_once_when_reachable() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let position = EntityPosition {
        x: 0.5,
        y: 64.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };

    let updates = manager
        .drop_item(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            position,
            crate::inventory::simple_item(1, 1),
        )
        .unwrap();
    assert_eq!(updates.len(), 1);

    std::thread::sleep(std::time::Duration::from_millis(550));
    let collected = manager
        .collect_reachable_items("minecraft:overworld", position)
        .unwrap();
    assert_eq!(collected.len(), 1);
    assert!(
        manager
            .collect_reachable_items("minecraft:overworld", position)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn dropped_items_can_be_settled_before_collection() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let high_position = EntityPosition {
        x: 0.5,
        y: 70.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: false,
    };
    let collector = EntityPosition {
        y: 64.0,
        on_ground: true,
        ..high_position
    };

    let updates = manager
        .drop_item(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            high_position,
            crate::inventory::simple_item(1, 1),
        )
        .unwrap();
    assert_eq!(updates.len(), 1);

    std::thread::sleep(std::time::Duration::from_millis(550));
    assert!(
        manager
            .collect_reachable_items("minecraft:overworld", collector)
            .unwrap()
            .is_empty()
    );

    manager.settle_collectable_dropped_items("minecraft:overworld", collector, |mut position| {
        position.y = 64.0;
        position
    });
    let collected = manager
        .collect_reachable_items("minecraft:overworld", collector)
        .unwrap();

    assert_eq!(collected.len(), 1);
}

#[test]
fn dropped_items_merge_until_configured_stack_limit() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let position = EntityPosition {
        x: 0.5,
        y: 64.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };
    let rendering = qexed_config::app::qexed::server::EntityRendering {
        item_merge_radius: 3.0,
        item_merge_max_stack: 64,
        ..Default::default()
    };

    let first = manager
        .drop_item_with_rendering(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            position,
            crate::inventory::simple_item(1, 32),
            &rendering,
        )
        .unwrap();
    let second = manager
        .drop_item_with_rendering(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            position,
            crate::inventory::simple_item(1, 40),
            &rendering,
        )
        .unwrap();

    assert!(matches!(
        first.as_slice(),
        [crate::entities::DroppedItemUpdate::Spawned(_)]
    ));
    assert_eq!(second.len(), 2);
    assert!(matches!(
        &second[0],
        crate::entities::DroppedItemUpdate::Merged(entity) if entity.item.item_count.0 == 64
    ));
    assert!(matches!(
        &second[1],
        crate::entities::DroppedItemUpdate::Spawned(entity) if entity.item.item_count.0 == 8
    ));
}

#[test]
fn runtime_entities_can_spawn_move_and_remove() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let position = EntityPosition {
        x: 1.0,
        y: 65.0,
        z: 2.0,
        yaw: 90.0,
        pitch: 0.0,
        on_ground: true,
    };

    let entity = manager
        .spawn(
            &players,
            EntitySpawnRequest {
                key: "guide".to_string(),
                kind: ManagedEntityKind::Npc,
                entity_type: String::new(),
                entity_type_id_override: None,
                dimension: "minecraft:overworld".to_string(),
                position,
                name: "Guide".to_string(),
                display_name: String::new(),
                skin_textures: String::new(),
                skin_signature: String::new(),
                data: 0,
                ai: String::new(),
                ai_params: Default::default(),
                auto_jump: false,
                spawn_rule: String::new(),
                custom_type: String::new(),
                look_at_players: false,
                main_hand_event: "interact".to_string(),
                off_hand_event: "interact_off_hand".to_string(),
                attack_event: "attack".to_string(),
            },
        )
        .unwrap();

    assert_eq!(entity.key, "guide");
    assert_eq!(
        manager.list_for_dimension("minecraft:overworld")[0].kind,
        ManagedEntityKind::Npc
    );

    let moved = EntityPosition { x: 3.0, ..position };
    manager.move_entity(&players, "guide", moved).unwrap();
    assert_eq!(
        manager.list_for_dimension("minecraft:overworld")[0]
            .position
            .x,
        3.0
    );

    manager.remove(&players, "guide").unwrap();
    assert!(manager.list_for_dimension("minecraft:overworld").is_empty());
}

#[test]
fn entity_view_simplifies_stacked_same_type_entities() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids,
    )
    .unwrap();
    let position = EntityPosition {
        x: 0.0,
        y: 64.0,
        z: 0.0,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };
    for index in 0..3 {
        manager
            .spawn_local(EntitySpawnRequest {
                key: format!("zombie-{index}"),
                kind: ManagedEntityKind::Entity,
                entity_type: "minecraft:zombie".to_string(),
                entity_type_id_override: None,
                dimension: "minecraft:overworld".to_string(),
                position: EntityPosition {
                    x: f64::from(index) * 0.5,
                    ..position
                },
                name: String::new(),
                display_name: String::new(),
                skin_textures: String::new(),
                skin_signature: String::new(),
                data: 0,
                ai: String::new(),
                ai_params: Default::default(),
                auto_jump: false,
                spawn_rule: String::new(),
                custom_type: String::new(),
                look_at_players: false,
                main_hand_event: "interact".to_string(),
                off_hand_event: "interact_off_hand".to_string(),
                attack_event: "attack".to_string(),
            })
            .unwrap();
    }

    let packets = manager
        .spawn_packets_for_view(
            "minecraft:overworld",
            position,
            &qexed_config::app::qexed::server::EntityRendering {
                default_distance: 64.0,
                stack_threshold: 3,
                stack_radius: 4.0,
                ..Default::default()
            },
        )
        .unwrap();

    let add_entity_count = packets
        .iter()
        .filter(|packet| {
            packet[0] == qexed_protocol::to_client::play::add_entity::AddEntity::ID as u8
        })
        .count();
    assert_eq!(add_entity_count, 1);
    let label = "zombie*3".as_bytes();
    assert!(
        packets
            .iter()
            .any(|packet| packet.windows(label.len()).any(|window| window == label)),
        "stacked entity view should include zombie*3 label"
    );
}

#[test]
fn spawn_rules_respect_caps_and_create_dynamic_entities() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Tester".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    let spawning = qexed_config::app::qexed::server::EntitySpawning {
        enable: true,
        tick_interval_ms: 50,
        ai_tick_interval_ms: 50,
        global_cap: 2,
        per_dimension_cap: 2,
        per_type_cap: 2,
        max_spawn_per_tick: 4,
        player_activation_range: 16.0,
        slime_chunks: Default::default(),
        rules: vec![qexed_config::app::qexed::server::EntitySpawnRule {
            id: "zombies".to_string(),
            dimension: "minecraft:overworld".to_string(),
            entity_type: "minecraft:zombie".to_string(),
            cap: 2,
            min_x: -1.0,
            max_x: 1.0,
            min_y: 64.0,
            max_y: 64.0,
            min_z: -1.0,
            max_z: 1.0,
            ..Default::default()
        }],
    };

    let spawned = manager
        .spawn_from_rules(
            &players,
            &world,
            &qexed_config::app::qexed::server::EntityRendering::default(),
            &spawning,
            "minecraft:overworld",
        )
        .unwrap();

    assert_eq!(spawned, 2);
    let entities = manager.list_for_dimension("minecraft:overworld");
    assert_eq!(entities.len(), 2);
    assert!(entities.iter().all(|entity| entity.spawn_rule == "zombies"));
    assert!(
        manager
            .spawn_from_rules(
                &players,
                &world,
                &qexed_config::app::qexed::server::EntityRendering::default(),
                &spawning,
                "minecraft:overworld",
            )
            .unwrap()
            <= 1
    );
}

#[test]
fn spawn_rules_apply_rule_conditions_and_position_checks() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Tester".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    world.place_block(
        "minecraft:overworld",
        qexed_packet::net_types::Position { x: 0, y: 63, z: 0 },
        stone_block_state(),
    );

    let spawning = qexed_config::app::qexed::server::EntitySpawning {
        enable: true,
        tick_interval_ms: 50,
        ai_tick_interval_ms: 50,
        global_cap: 4,
        per_dimension_cap: 4,
        per_type_cap: 4,
        max_spawn_per_tick: 4,
        player_activation_range: 1.0,
        slime_chunks: Default::default(),
        rules: vec![qexed_config::app::qexed::server::EntitySpawnRule {
            id: "grounded".to_string(),
            dimension: "minecraft:overworld".to_string(),
            entity_type: "minecraft:zombie".to_string(),
            cap: 4,
            tick_interval_ms: 60_000,
            require_ground: true,
            require_air: true,
            position_attempts: 1,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 64.0,
            max_y: 64.0,
            min_z: 0.0,
            max_z: 0.0,
            ..Default::default()
        }],
    };

    assert_eq!(
        manager
            .spawn_from_rules(
                &players,
                &world,
                &qexed_config::app::qexed::server::EntityRendering::default(),
                &spawning,
                "minecraft:overworld",
            )
            .unwrap(),
        1
    );
    assert_eq!(
        manager
            .spawn_from_rules(
                &players,
                &world,
                &qexed_config::app::qexed::server::EntityRendering::default(),
                &spawning,
                "minecraft:overworld",
            )
            .unwrap(),
        0
    );
}

#[test]
fn spawn_rules_merge_custom_entity_ai_params() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    manager.register_custom_entities([crate::plugins::CustomEntityDefinition {
        id: "demo:patrol_guard".to_string(),
        entity_type: String::new(),
        shell_entity_type: "minecraft:villager".to_string(),
        registry_id: None,
        display_name: "Guard".to_string(),
        ai: "plugin:demo_patrol".to_string(),
        ai_params: [
            ("iq".to_string(), serde_json::json!(100)),
            ("patrol_min_x".to_string(), serde_json::json!(4.0)),
        ]
        .into_iter()
        .collect(),
    }]);
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Tester".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let spawning = qexed_config::app::qexed::server::EntitySpawning {
        enable: true,
        tick_interval_ms: 50,
        ai_tick_interval_ms: 50,
        global_cap: 1,
        per_dimension_cap: 1,
        per_type_cap: 1,
        max_spawn_per_tick: 1,
        player_activation_range: 16.0,
        slime_chunks: Default::default(),
        rules: vec![qexed_config::app::qexed::server::EntitySpawnRule {
            id: "guards".to_string(),
            dimension: "minecraft:overworld".to_string(),
            entity_type: "demo:patrol_guard".to_string(),
            cap: 1,
            require_ground: false,
            require_air: false,
            ai_params: [("iq".to_string(), serde_json::json!(114514))]
                .into_iter()
                .collect(),
            min_x: 0.0,
            max_x: 0.0,
            min_y: 64.0,
            max_y: 64.0,
            min_z: 0.0,
            max_z: 0.0,
            ..Default::default()
        }],
    };

    let world = empty_world();
    let spawned = manager
        .spawn_from_rules(
            &players,
            &world,
            &qexed_config::app::qexed::server::EntityRendering::default(),
            &spawning,
            "minecraft:overworld",
        )
        .unwrap();

    assert_eq!(spawned, 1);
    let entity = manager.list_for_dimension("minecraft:overworld").remove(0);
    assert_eq!(entity.entity_type, "minecraft:villager");
    assert_eq!(entity.custom_type, "demo:patrol_guard");
    assert_eq!(entity.ai, "plugin:demo_patrol");
    assert_eq!(entity.ai_params["iq"], 114514);
    assert_eq!(entity.ai_params["patrol_min_x"], 4.0);
}

#[test]
fn custom_entity_without_shell_uses_custom_id_as_client_entity_type() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    manager.register_custom_entities([crate::plugins::CustomEntityDefinition {
        id: "demo:modded_guard".to_string(),
        display_name: "Guard".to_string(),
        ai: "plugin:demo_patrol".to_string(),
        ..Default::default()
    }]);
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Tester".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let spawning = qexed_config::app::qexed::server::EntitySpawning {
        enable: true,
        tick_interval_ms: 50,
        ai_tick_interval_ms: 50,
        global_cap: 1,
        per_dimension_cap: 1,
        per_type_cap: 1,
        max_spawn_per_tick: 1,
        player_activation_range: 16.0,
        slime_chunks: Default::default(),
        rules: vec![qexed_config::app::qexed::server::EntitySpawnRule {
            id: "guards".to_string(),
            dimension: "minecraft:overworld".to_string(),
            entity_type: "demo:modded_guard".to_string(),
            cap: 1,
            require_ground: false,
            require_air: false,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 64.0,
            max_y: 64.0,
            min_z: 0.0,
            max_z: 0.0,
            ..Default::default()
        }],
    };

    let spawned = manager
        .spawn_from_rules(
            &players,
            &empty_world(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            &spawning,
            "minecraft:overworld",
        )
        .unwrap();

    assert_eq!(spawned, 1);
    let entity = manager.list_for_dimension("minecraft:overworld").remove(0);
    assert_eq!(entity.entity_type, "demo:modded_guard");
    assert_eq!(entity.custom_type, "demo:modded_guard");
    assert_eq!(entity.ai, "plugin:demo_patrol");
}

#[test]
fn follow_nearest_player_ai_moves_entity_toward_player() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Target".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 10.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    world.place_block(
        "minecraft:overworld",
        qexed_packet::net_types::Position { x: 0, y: 63, z: 0 },
        stone_block_state(),
    );
    manager
        .spawn_local(EntitySpawnRequest {
            key: "follower".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Follower".to_string(),
            display_name: "Follower".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();

    let entity = manager.entity_by_key("follower").unwrap();
    assert!(entity.position.x > 0.0);
    assert_eq!(entity.position.z, 0.0);
}

#[test]
fn vanilla_hostile_ai_moves_entity_toward_player() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Target".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 10.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    world.place_block(
        "minecraft:overworld",
        qexed_packet::net_types::Position { x: 0, y: 63, z: 0 },
        stone_block_state(),
    );
    manager
        .spawn_local(EntitySpawnRequest {
            key: "vanilla-zombie".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Zombie".to_string(),
            display_name: "Zombie".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();

    let entity = manager.entity_by_key("vanilla-zombie").unwrap();
    assert!(entity.position.x > 0.0);
    assert_eq!(entity.position.z, 0.0);
}

#[test]
fn vanilla_zombie_melee_ai_damages_nearby_player() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let mut session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 1.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    manager
        .spawn_local(EntitySpawnRequest {
            key: "attacking-zombie".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Zombie".to_string(),
            display_name: "Zombie".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();

    match session.receiver.try_recv().expect("player damage event") {
        crate::players::PlayerEvent::Damage {
            profile_id,
            amount,
            kind,
            source_entity_id,
            ..
        } => {
            assert_eq!(profile_id, session.player.profile.uuid);
            assert_eq!(amount, 3.0);
            assert_eq!(kind, crate::players::PlayerDamageKind::MobAttack);
            assert_eq!(
                source_entity_id,
                manager.entity_by_key("attacking-zombie").unwrap().entity_id
            );
        }
        event => panic!("expected damage event, got {event:?}"),
    }
}

#[test]
fn vanilla_creeper_swell_explodes_and_damages_player() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let mut session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 2.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    manager
        .spawn_local(EntitySpawnRequest {
            key: "exploding-creeper".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:creeper".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Creeper".to_string(),
            display_name: "Creeper".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            1_500,
        )
        .unwrap();

    match session
        .receiver
        .try_recv()
        .expect("player explosion damage event")
    {
        crate::players::PlayerEvent::Damage { amount, kind, .. } => {
            assert!(amount > 0.0);
            assert_eq!(kind, crate::players::PlayerDamageKind::Explosion);
        }
        event => panic!("expected damage event, got {event:?}"),
    }
    assert!(manager.entity_by_key("exploding-creeper").is_none());
}

#[test]
fn vanilla_creeper_explosion_breaks_blocks() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 2.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    let block = qexed_packet::net_types::Position { x: 1, y: 64, z: 0 };
    world.place_block("minecraft:overworld", block.clone(), stone_block_state());
    manager
        .spawn_local(EntitySpawnRequest {
            key: "block-breaking-creeper".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:creeper".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Creeper".to_string(),
            display_name: "Creeper".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            1_500,
        )
        .unwrap();

    assert!(crate::inventory::is_air_block_state(
        world.block_state_at("minecraft:overworld", &block).unwrap()
    ));
}

#[test]
fn vanilla_creeper_explosion_can_disable_block_breaking() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 2.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    let block = qexed_packet::net_types::Position { x: 1, y: 64, z: 0 };
    let stone = stone_block_state();
    world.place_block("minecraft:overworld", block.clone(), stone);
    let mut ai_params = std::collections::BTreeMap::new();
    ai_params.insert(
        "creeper_break_blocks".to_string(),
        serde_json::Value::Bool(false),
    );
    manager
        .spawn_local(EntitySpawnRequest {
            key: "non-breaking-creeper".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:creeper".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Creeper".to_string(),
            display_name: "Creeper".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params,
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            1_500,
        )
        .unwrap();

    assert_eq!(
        world.block_state_at("minecraft:overworld", &block),
        Some(stone)
    );
}

#[test]
fn vanilla_creeper_explosion_damages_nearby_entity() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(2.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "creeper-victim-zombie",
        "minecraft:zombie",
        test_position(1.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "entity-damaging-creeper",
        "minecraft:creeper",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            1_500,
        )
        .unwrap();

    assert!(
        manager
            .entity_health_for_tests("creeper-victim-zombie")
            .is_some_and(|health| health < 20.0)
    );
}

#[test]
fn vanilla_skeleton_ranged_ai_spawns_arrow_projectile() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 10.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    manager
        .spawn_local(EntitySpawnRequest {
            key: "ranged-skeleton".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:skeleton".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Skeleton".to_string(),
            display_name: "Skeleton".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:arrow"
                && entity.ai == "vanilla_projectile:arrow")
    );
}

#[test]
fn vanilla_arrow_projectile_hits_player_and_is_removed() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let mut session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 10.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    manager
        .spawn_local(EntitySpawnRequest {
            key: "shooting-skeleton".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:skeleton".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Skeleton".to_string(),
            display_name: "Skeleton".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();
    while session.receiver.try_recv().is_ok() {}
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut damage_event = None;
    for _ in 0..8 {
        manager
            .tick_ai(
                &players,
                &world,
                &crate::plugins::PluginManager::empty_for_tests(),
                &qexed_config::app::qexed::server::EntityRendering::default(),
                50,
            )
            .unwrap();
        while let Ok(event) = session.receiver.try_recv() {
            if matches!(event, crate::players::PlayerEvent::Damage { .. }) {
                damage_event = Some(event);
                break;
            }
        }
        if damage_event.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    match damage_event.expect("arrow damage event") {
        crate::players::PlayerEvent::Damage { amount, kind, .. } => {
            assert_eq!(amount, 4.0);
            assert_eq!(kind, crate::players::PlayerDamageKind::Projectile);
        }
        event => panic!("expected damage event, got {event:?}"),
    }
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .all(|entity| entity.entity_type != "minecraft:arrow")
    );
}

#[test]
fn vanilla_blaze_ranged_ai_spawns_small_fireball_projectile() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(8.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "ranged-blaze",
        "minecraft:blaze",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:small_fireball"
                && entity.ai == "vanilla_projectile:small_fireball")
    );
}

#[test]
fn vanilla_small_fireball_projectile_hits_player() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(6.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "shooting-blaze",
        "minecraft:blaze",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);
    drain_player_events(&mut session);
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut damage_event = None;
    for _ in 0..10 {
        tick_entities(&manager, &players, &world);
        while let Ok(event) = session.receiver.try_recv() {
            if matches!(event, crate::players::PlayerEvent::Damage { .. }) {
                damage_event = Some(event);
                break;
            }
        }
        if damage_event.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    match damage_event.expect("small fireball damage event") {
        crate::players::PlayerEvent::Damage { amount, kind, .. } => {
            assert_eq!(amount, 5.0);
            assert_eq!(kind, crate::players::PlayerDamageKind::Projectile);
        }
        event => panic!("expected damage event, got {event:?}"),
    }
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .all(|entity| entity.entity_type != "minecraft:small_fireball")
    );
}

#[test]
fn vanilla_witch_potion_projectile_applies_instant_damage_effect() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(4.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "throwing-witch",
        "minecraft:witch",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:splash_potion"
                && entity.ai == "vanilla_projectile:potion")
    );
    drain_player_events(&mut session);
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut potion_event = None;
    for _ in 0..12 {
        tick_entities(&manager, &players, &world);
        while let Ok(event) = session.receiver.try_recv() {
            if matches!(event, crate::players::PlayerEvent::PotionEffect { .. }) {
                potion_event = Some(event);
                break;
            }
        }
        if potion_event.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    match potion_event.expect("witch potion effect event") {
        crate::players::PlayerEvent::PotionEffect {
            effect,
            amplifier,
            duration_ticks,
            ..
        } => {
            assert_eq!(effect, "minecraft:instant_damage");
            assert_eq!(amplifier, 0);
            assert_eq!(duration_ticks, 1);
        }
        event => panic!("expected potion effect event, got {event:?}"),
    }
}

#[test]
fn vanilla_ghast_fireball_explodes_and_breaks_blocks() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(8.0, 64.0, 0.0));
    let world = empty_world();
    let target_block = qexed_packet::net_types::Position { x: 8, y: 64, z: 0 };
    world.place_block(
        "minecraft:overworld",
        target_block.clone(),
        stone_block_state(),
    );
    spawn_vanilla_entity(
        &manager,
        "shooting-ghast",
        "minecraft:ghast",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:fireball"
                && entity.ai == "vanilla_projectile:fireball")
    );
    drain_player_events(&mut session);
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut damage_event = None;
    for _ in 0..12 {
        tick_entities(&manager, &players, &world);
        while let Ok(event) = session.receiver.try_recv() {
            if matches!(event, crate::players::PlayerEvent::Damage { .. }) {
                damage_event = Some(event);
                break;
            }
        }
        if damage_event.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    match damage_event.expect("ghast fireball explosion damage event") {
        crate::players::PlayerEvent::Damage { kind, .. } => {
            assert_eq!(kind, crate::players::PlayerDamageKind::Explosion);
        }
        event => panic!("expected damage event, got {event:?}"),
    }
    let current = world
        .block_state_at("minecraft:overworld", &target_block)
        .expect("target block state");
    assert!(crate::inventory::is_air_block_state(current));
}

#[test]
fn vanilla_ghast_fireball_can_disable_block_breaking() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(8.0, 64.0, 0.0));
    let world = empty_world();
    let target_block = qexed_packet::net_types::Position { x: 8, y: 64, z: 0 };
    let stone = stone_block_state();
    world.place_block("minecraft:overworld", target_block.clone(), stone);
    let mut ai_params = BTreeMap::new();
    ai_params.insert(
        "projectile_break_blocks".to_string(),
        serde_json::json!(false),
    );
    spawn_vanilla_entity(
        &manager,
        "nonbreaking-ghast",
        "minecraft:ghast",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);
    drain_player_events(&mut session);
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut damage_event = None;
    for _ in 0..12 {
        tick_entities(&manager, &players, &world);
        while let Ok(event) = session.receiver.try_recv() {
            if matches!(event, crate::players::PlayerEvent::Damage { .. }) {
                damage_event = Some(event);
                break;
            }
        }
        if damage_event.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    assert!(damage_event.is_some());
    let current = world
        .block_state_at("minecraft:overworld", &target_block)
        .expect("target block state");
    assert_eq!(current, stone);
}

#[test]
fn vanilla_shulker_ranged_ai_spawns_shulker_bullet_projectile() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(8.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "ranged-shulker",
        "minecraft:shulker",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:shulker_bullet"
                && entity.ai == "vanilla_projectile:shulker_bullet")
    );
}

#[test]
fn vanilla_shulker_bullet_tracks_moved_target() {
    let (manager, players) = test_manager_and_players();
    let session = join_test_player(&players, "Target", test_position(10.0, 64.0, 0.0));
    let world = empty_world();
    let mut ai_params = BTreeMap::new();
    ai_params.insert("projectile_speed".to_string(), serde_json::json!(0.4));
    spawn_vanilla_entity(
        &manager,
        "tracking-shulker",
        "minecraft:shulker",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);
    let spawned_bullet = manager
        .list_for_dimension("minecraft:overworld")
        .into_iter()
        .find(|entity| entity.entity_type == "minecraft:shulker_bullet")
        .expect("shulker bullet spawned");

    players.update_position(session.player.profile.uuid, test_position(10.0, 64.0, 6.0));

    let mut tracked_bullet = spawned_bullet.clone();
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(60));
        tick_entities(&manager, &players, &world);
        tracked_bullet = manager
            .list_for_dimension("minecraft:overworld")
            .into_iter()
            .find(|entity| entity.key == spawned_bullet.key)
            .expect("shulker bullet remains active");
    }

    assert!(
        tracked_bullet.position.z > spawned_bullet.position.z + 0.15,
        "expected shulker bullet to bend toward moved target: start_z={}, tracked_z={}",
        spawned_bullet.position.z,
        tracked_bullet.position.z
    );
    assert!(tracked_bullet.position.x > spawned_bullet.position.x);
}

#[test]
fn vanilla_shulker_bullet_hits_player_with_damage_and_levitation() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(4.0, 64.0, 0.0));
    let world = empty_world();
    let mut ai_params = BTreeMap::new();
    ai_params.insert("projectile_speed".to_string(), serde_json::json!(0.8));
    spawn_vanilla_entity(
        &manager,
        "shooting-shulker",
        "minecraft:shulker",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);
    drain_player_events(&mut session);
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut damage = None;
    let mut effect = None;
    for _ in 0..10 {
        tick_entities(&manager, &players, &world);
        while let Ok(event) = session.receiver.try_recv() {
            match event {
                crate::players::PlayerEvent::Damage { amount, kind, .. } => {
                    damage = Some((amount, kind));
                }
                crate::players::PlayerEvent::PotionEffect {
                    effect: effect_name,
                    duration_ticks,
                    ..
                } => {
                    effect = Some((effect_name, duration_ticks));
                }
                _ => {}
            }
        }
        if damage.is_some() && effect.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    assert_eq!(
        damage,
        Some((4.0, crate::players::PlayerDamageKind::Projectile))
    );
    assert_eq!(effect, Some(("minecraft:levitation".to_string(), 20 * 10)));
}

#[test]
fn vanilla_breeze_ranged_ai_spawns_wind_charge_projectile() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(8.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "ranged-breeze",
        "minecraft:breeze",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:wind_charge"
                && entity.ai == "vanilla_projectile:wind_charge")
    );
}

#[test]
fn vanilla_wither_ranged_ai_spawns_wither_skull_projectile() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(12.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "ranged-wither",
        "minecraft:wither",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:wither_skull"
                && entity.ai == "vanilla_projectile:wither_skull")
    );
}

#[test]
fn vanilla_cave_spider_melee_applies_poison() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(1.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "poison-spider",
        "minecraft:cave_spider",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let mut damaged = false;
    let mut poison = None;
    while let Ok(event) = session.receiver.try_recv() {
        match event {
            crate::players::PlayerEvent::Damage { kind, .. } => {
                damaged = kind == crate::players::PlayerDamageKind::MobAttack;
            }
            crate::players::PlayerEvent::PotionEffect {
                effect,
                duration_ticks,
                ..
            } => {
                poison = Some((effect, duration_ticks));
            }
            _ => {}
        }
    }

    assert!(damaged);
    assert_eq!(poison, Some(("minecraft:poison".to_string(), 20 * 7)));
}

#[test]
fn vanilla_wither_skeleton_melee_applies_wither() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(1.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "wither-skeleton",
        "minecraft:wither_skeleton",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let mut effect = None;
    while let Ok(event) = session.receiver.try_recv() {
        if let crate::players::PlayerEvent::PotionEffect {
            effect: effect_name,
            duration_ticks,
            ..
        } = event
        {
            effect = Some((effect_name, duration_ticks));
        }
    }

    assert_eq!(effect, Some(("minecraft:wither".to_string(), 20 * 10)));
}

#[test]
fn vanilla_husk_melee_applies_hunger() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(1.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "hungry-husk",
        "minecraft:husk",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let mut effect = None;
    while let Ok(event) = session.receiver.try_recv() {
        if let crate::players::PlayerEvent::PotionEffect {
            effect: effect_name,
            duration_ticks,
            ..
        } = event
        {
            effect = Some((effect_name, duration_ticks));
        }
    }

    assert_eq!(effect, Some(("minecraft:hunger".to_string(), 20 * 7)));
}

#[test]
fn vanilla_guardian_charges_magic_beam_damage() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(8.0, 64.0, 0.0));
    let world = empty_world();
    let mut ai_params = BTreeMap::new();
    ai_params.insert("attack_duration_ticks".to_string(), serde_json::json!(2));
    ai_params.insert("guardian_magic_damage".to_string(), serde_json::json!(3.0));
    spawn_vanilla_entity(
        &manager,
        "beam-guardian",
        "minecraft:guardian",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);
    drain_player_events(&mut session);
    std::thread::sleep(std::time::Duration::from_millis(60));
    tick_entities(&manager, &players, &world);

    let mut damage = None;
    while let Ok(event) = session.receiver.try_recv() {
        if let crate::players::PlayerEvent::Damage { amount, kind, .. } = event {
            damage = Some((amount, kind));
        }
    }

    assert_eq!(damage, Some((3.0, crate::players::PlayerDamageKind::Magic)));
}

#[test]
fn vanilla_zombie_burns_in_daylight() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Viewer".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    let world_rules = crate::world::WorldRulesManager::from_world_config_for_tests(
        &qexed_config::app::qexed::server::World::default(),
        tempfile::tempdir().expect("world rules dir").keep(),
    )
    .unwrap();
    world_rules
        .set_time_value("minecraft:overworld", 1_000)
        .unwrap();
    manager
        .spawn_local(EntitySpawnRequest {
            key: "daylight-zombie".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Zombie".to_string(),
            display_name: "Zombie".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "vanilla".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai_with_world_rules(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            Some(&world_rules),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            1_000,
        )
        .unwrap();

    assert!(
        manager
            .entity_fire_ticks_for_tests("daylight-zombie")
            .is_some_and(|ticks| ticks > 0)
    );
    assert!(
        manager
            .entity_health_for_tests("daylight-zombie")
            .is_some_and(|health| health < 20.0)
    );
}

#[test]
fn vanilla_enderman_teleports_towards_far_target() {
    let (manager, players) = test_manager_and_players();
    let mut target_position = test_position(24.0, 64.0, 0.0);
    target_position.yaw = 90.0;
    let _session = join_test_player(&players, "Target", target_position);
    let world = empty_world();
    place_stone_floor(&world, -1, 1, -1, 1, 63);
    place_stone_floor(&world, 18, 22, -1, 1, 63);
    let mut ai_params = BTreeMap::new();
    ai_params.insert(
        "enderman_teleport_chance".to_string(),
        serde_json::json!(1.0),
    );
    ai_params.insert(
        "enderman_teleport_min_distance".to_string(),
        serde_json::json!(8.0),
    );
    ai_params.insert(
        "enderman_teleport_arrival_distance".to_string(),
        serde_json::json!(4.0),
    );
    spawn_vanilla_entity(
        &manager,
        "teleporting-enderman",
        "minecraft:enderman",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("teleporting-enderman").unwrap();
    assert!(
        (18.0..=22.0).contains(&entity.position.x),
        "enderman should teleport near the player-side landing spot, got {:?}",
        entity.position
    );
    assert!((entity.position.y - 64.0).abs() < 0.001);
    assert!(entity.position.on_ground);
}

#[test]
fn vanilla_enderman_ignores_player_until_stared_at() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(1.0, 64.0, 0.0));
    let world = empty_world();
    let mut ai_params = BTreeMap::new();
    ai_params.insert("stroll_chance".to_string(), serde_json::json!(0.0));
    spawn_vanilla_entity(
        &manager,
        "calm-enderman",
        "minecraft:enderman",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);

    let mut damaged = false;
    while let Ok(event) = session.receiver.try_recv() {
        if matches!(event, crate::players::PlayerEvent::Damage { .. }) {
            damaged = true;
        }
    }
    assert!(
        !damaged,
        "enderman should not attack a player who is not looking at it"
    );
}

#[test]
fn vanilla_enderman_attacks_player_staring_at_it() {
    let (manager, players) = test_manager_and_players();
    let mut target_position = test_position(1.0, 64.0, 0.0);
    target_position.yaw = 90.0;
    let mut session = join_test_player(&players, "Target", target_position);
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "stared-enderman",
        "minecraft:enderman",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let mut damage = None;
    while let Ok(event) = session.receiver.try_recv() {
        if let crate::players::PlayerEvent::Damage { amount, kind, .. } = event {
            damage = Some((amount, kind));
        }
    }
    assert_eq!(
        damage,
        Some((7.0, crate::players::PlayerDamageKind::MobAttack))
    );
}

#[test]
fn vanilla_slime_jumps_while_chasing_player() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(5.0, 64.0, 0.0));
    let world = empty_world();
    place_stone_floor(&world, -1, 6, -1, 1, 63);
    spawn_vanilla_entity(
        &manager,
        "jumping-slime",
        "minecraft:slime",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("jumping-slime").unwrap();
    assert!(
        entity.position.y > 64.0,
        "slime should hop while moving toward a target, got {:?}",
        entity.position
    );
    assert!(!entity.position.on_ground);
}

#[test]
fn vanilla_slime_splits_on_death() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(4.0, 64.0, 0.0));
    let mut ai_params = BTreeMap::new();
    ai_params.insert("slime_size".to_string(), serde_json::json!(4));
    ai_params.insert("slime_split_count".to_string(), serde_json::json!(3));
    spawn_vanilla_entity(
        &manager,
        "splitting-slime",
        "minecraft:slime",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );
    let entity_id = manager.entity_by_key("splitting-slime").unwrap().entity_id;

    let result = manager
        .damage_managed_entity(
            &players,
            &qexed_config::app::qexed::server::EntityRendering::default(),
            entity_id,
            100.0,
        )
        .unwrap()
        .expect("damage result");

    assert!(result.killed);
    assert!(manager.entity_by_key("splitting-slime").is_some());
    let children = manager
        .list_for_dimension("minecraft:overworld")
        .into_iter()
        .filter(|entity| entity.key.starts_with("splitting-slime:split:"))
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3);
    for child in children {
        assert_eq!(child.entity_type, "minecraft:slime");
        assert_eq!(child.data, 2);
        assert_eq!(manager.entity_health_for_tests(&child.key), Some(4.0));
    }
}

#[test]
fn vanilla_magma_cube_jump_scales_with_size() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(5.0, 64.0, 0.0));
    let world = empty_world();
    place_stone_floor(&world, -1, 6, -1, 1, 63);
    let mut ai_params = BTreeMap::new();
    ai_params.insert("slime_size".to_string(), serde_json::json!(4));
    spawn_vanilla_entity(
        &manager,
        "jumping-magma-cube",
        "minecraft:magma_cube",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("jumping-magma-cube").unwrap();
    assert!(
        entity.position.y > 64.7,
        "magma cube size boost should produce a higher jump, got {:?}",
        entity.position
    );
    assert!(!entity.position.on_ground);
}

#[test]
fn vanilla_spider_climbs_when_colliding_with_wall() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(4.0, 64.0, 0.0));
    let world = empty_world();
    place_stone_floor(&world, -1, 4, 0, 0, 63);
    for y in 64..=66 {
        for z in -1..=1 {
            world.set_runtime_block(
                "minecraft:overworld",
                qexed_packet::net_types::Position { x: 1, y, z },
                stone_block_state(),
            );
        }
    }
    spawn_vanilla_entity(
        &manager,
        "climbing-spider",
        "minecraft:spider",
        test_position(0.65, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("climbing-spider").unwrap();
    assert!(
        entity.position.y > 64.0,
        "spider should climb while pressing into a wall, got {:?}",
        entity.position
    );
    assert!(!entity.position.on_ground);
}

#[test]
fn vanilla_phantom_flies_without_falling_in_open_air() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(6.0, 70.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "flying-phantom",
        "minecraft:phantom",
        test_position(0.0, 70.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("flying-phantom").unwrap();
    assert!(
        entity.position.y >= 69.99,
        "phantom should use flying physics instead of gravity, got {:?}",
        entity.position
    );
}

#[test]
fn vanilla_drowned_with_trident_throws_trident_projectile() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Target", test_position(10.0, 64.0, 0.0));
    let world = empty_world();
    let mut ai_params = BTreeMap::new();
    ai_params.insert("drowned_has_trident".to_string(), serde_json::json!(true));
    spawn_vanilla_entity(
        &manager,
        "trident-drowned",
        "minecraft:drowned",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:trident"
                && entity.ai == "vanilla_projectile:trident")
    );
}

#[test]
fn vanilla_evoker_spell_spawns_fangs_and_damages_player() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(6.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "spell-evoker",
        "minecraft:evoker",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let mut damage = None;
    while let Ok(event) = session.receiver.try_recv() {
        if let crate::players::PlayerEvent::Damage { amount, kind, .. } = event {
            damage = Some((amount, kind));
        }
    }
    assert_eq!(damage, Some((6.0, crate::players::PlayerDamageKind::Magic)));
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:evoker_fangs")
    );
}

#[test]
fn vanilla_neutral_wolf_attacks_only_when_angry() {
    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(1.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "calm-wolf",
        "minecraft:wolf",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let mut calm_damage = false;
    while let Ok(event) = session.receiver.try_recv() {
        if matches!(event, crate::players::PlayerEvent::Damage { .. }) {
            calm_damage = true;
        }
    }
    assert!(!calm_damage);

    let (manager, players) = test_manager_and_players();
    let mut session = join_test_player(&players, "Target", test_position(1.0, 64.0, 0.0));
    let mut ai_params = BTreeMap::new();
    ai_params.insert("angry".to_string(), serde_json::json!(true));
    spawn_vanilla_entity(
        &manager,
        "angry-wolf",
        "minecraft:wolf",
        test_position(0.0, 64.0, 0.0),
        ai_params,
    );

    tick_entities(&manager, &players, &world);

    let mut angry_damage = false;
    while let Ok(event) = session.receiver.try_recv() {
        if matches!(
            event,
            crate::players::PlayerEvent::Damage { amount: 4.0, .. }
        ) {
            angry_damage = true;
        }
    }
    assert!(angry_damage);
}

#[test]
fn vanilla_zombie_damages_villager_entity() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(6.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "zombie-villager-target",
        "minecraft:villager",
        test_position(1.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "villager-hunting-zombie",
        "minecraft:zombie",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .entity_health_for_tests("zombie-villager-target")
            .is_some_and(|health| health < 20.0)
    );
}

#[test]
fn vanilla_villager_flees_nearby_zombie() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(10.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "fleeing-villager",
        "minecraft:villager",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "villager-threat-zombie",
        "minecraft:zombie",
        test_position(3.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("fleeing-villager").unwrap();
    assert!(
        entity.position.x < -0.01,
        "villager should move away from zombie on the x axis, got {:?}",
        entity.position
    );
}

#[test]
fn vanilla_creeper_flees_nearby_cat() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(10.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "cat-fearing-creeper",
        "minecraft:creeper",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "nearby-cat",
        "minecraft:cat",
        test_position(3.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("cat-fearing-creeper").unwrap();
    assert!(
        entity.position.x < -0.01,
        "creeper should move away from nearby cats, got {:?}",
        entity.position
    );
}

#[test]
fn vanilla_creeper_does_not_swell_near_cat() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        test_position(2.0, 64.0, 0.0),
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "cat-fearing-creeper",
        "minecraft:creeper",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "nearby-cat",
        "minecraft:cat",
        test_position(1.0, 64.0, 0.0),
        Default::default(),
    );

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            1_500,
        )
        .unwrap();

    assert!(manager.entity_by_key("cat-fearing-creeper").is_some());
    assert_eq!(
        manager.entity_health_for_tests("cat-fearing-creeper"),
        Some(20.0)
    );
}

#[test]
fn vanilla_skeleton_flees_nearby_wolf() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(10.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "wolf-fearing-skeleton",
        "minecraft:skeleton",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "nearby-wolf",
        "minecraft:wolf",
        test_position(3.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("wolf-fearing-skeleton").unwrap();
    assert!(
        entity.position.x < -0.01,
        "skeleton should move away from nearby wolves, got {:?}",
        entity.position
    );
}

#[test]
fn vanilla_rabbit_flees_nearby_fox() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(10.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "fox-fearing-rabbit",
        "minecraft:rabbit",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "nearby-fox",
        "minecraft:fox",
        test_position(3.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    let entity = manager.entity_by_key("fox-fearing-rabbit").unwrap();
    assert!(
        entity.position.x < -0.01,
        "rabbit should move away from nearby foxes, got {:?}",
        entity.position
    );
}

#[test]
fn vanilla_fox_attacks_rabbit_prey() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(6.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "hunting-fox",
        "minecraft:fox",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "fox-prey-rabbit",
        "minecraft:rabbit",
        test_position(1.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .entity_health_for_tests("fox-prey-rabbit")
            .is_some_and(|health| health < 6.0),
        "fox should damage nearby rabbit prey"
    );
}

#[test]
fn vanilla_wolf_attacks_skeleton_prey() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(6.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "skeleton-hunting-wolf",
        "minecraft:wolf",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "wolf-prey-skeleton",
        "minecraft:skeleton",
        test_position(1.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .entity_health_for_tests("wolf-prey-skeleton")
            .is_some_and(|health| health < 20.0),
        "wolf should damage nearby skeleton prey"
    );
}

#[test]
fn vanilla_iron_golem_attacks_hostile_entity_without_angry_flag() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(6.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "defending-golem",
        "minecraft:iron_golem",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "golem-target-zombie",
        "minecraft:zombie",
        test_position(1.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .entity_health_for_tests("golem-target-zombie")
            .is_some_and(|health| health < 20.0)
    );
}

#[test]
fn vanilla_snow_golem_throws_snowball_at_hostile_entity() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(12.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "defending-snow-golem",
        "minecraft:snow_golem",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "snow-golem-target-zombie",
        "minecraft:zombie",
        test_position(6.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:snowball"
                && entity.ai == "vanilla_projectile:snowball")
    );
}

#[test]
fn vanilla_arrow_projectile_hits_managed_entity_and_is_removed() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(12.0, 64.0, 0.0));
    let world = empty_world();
    let mut villager_params = BTreeMap::new();
    villager_params.insert(
        "disable_avoid_hostiles".to_string(),
        serde_json::json!(true),
    );
    spawn_vanilla_entity(
        &manager,
        "arrow-projectile-target-villager",
        "minecraft:villager",
        test_position(6.0, 64.0, 0.0),
        villager_params,
    );
    spawn_vanilla_entity(
        &manager,
        "arrow-projectile-pillager",
        "minecraft:pillager",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:arrow"),
        "pillager should spawn an arrow projectile"
    );
    assert_eq!(
        manager
            .entity_health_for_tests("arrow-projectile-target-villager")
            .unwrap(),
        16.0
    );

    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(60));
        tick_entities(&manager, &players, &world);
        if manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .all(|entity| entity.entity_type != "minecraft:arrow")
        {
            break;
        }
    }

    assert!(
        manager
            .entity_health_for_tests("arrow-projectile-target-villager")
            .is_some_and(|health| health <= 12.0),
        "managed entity should take projectile impact damage"
    );
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .all(|entity| entity.entity_type != "minecraft:arrow"),
        "arrow should be removed after hitting a managed entity"
    );
}

#[test]
fn vanilla_snowball_projectile_hits_managed_entity_and_is_removed() {
    let (manager, players) = test_manager_and_players();
    let _session = join_test_player(&players, "Viewer", test_position(12.0, 64.0, 0.0));
    let world = empty_world();
    spawn_vanilla_entity(
        &manager,
        "snowball-projectile-golem",
        "minecraft:snow_golem",
        test_position(0.0, 64.0, 0.0),
        Default::default(),
    );
    spawn_vanilla_entity(
        &manager,
        "snowball-projectile-target-zombie",
        "minecraft:zombie",
        test_position(6.0, 64.0, 0.0),
        Default::default(),
    );

    tick_entities(&manager, &players, &world);
    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .any(|entity| entity.entity_type == "minecraft:snowball"),
        "snow golem should spawn a snowball projectile"
    );

    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(60));
        tick_entities(&manager, &players, &world);
        if manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .all(|entity| entity.entity_type != "minecraft:snowball")
        {
            break;
        }
    }

    assert!(
        manager
            .list_for_dimension("minecraft:overworld")
            .iter()
            .all(|entity| entity.entity_type != "minecraft:snowball"),
        "0-damage snowballs should still despawn on managed entity impact"
    );
}

#[test]
fn follow_nearest_player_ai_accelerates_smoothly() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Target".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 10.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    for x in 0..=3 {
        world.place_block(
            "minecraft:overworld",
            qexed_packet::net_types::Position { x, y: 63, z: 0 },
            stone_block_state(),
        );
    }
    manager
        .spawn_local(EntitySpawnRequest {
            key: "smooth_follower".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Smooth".to_string(),
            display_name: "Smooth".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();
    let first = manager.entity_by_key("smooth_follower").unwrap().position.x;
    std::thread::sleep(std::time::Duration::from_millis(60));
    manager
        .tick_ai(
            &players,
            &world,
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();
    let second = manager.entity_by_key("smooth_follower").unwrap().position.x;

    assert!(first > 0.0);
    assert!(second - first > first);
}

#[test]
fn follow_nearest_player_ai_uses_path_around_wall() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 4.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    for x in -1..=4 {
        for z in -2..=2 {
            world.set_runtime_block(
                "minecraft:overworld",
                qexed_packet::net_types::Position { x, y: 63, z },
                stone_block_state(),
            );
        }
    }
    for z in -1..=1 {
        for y in 64..=65 {
            world.place_block(
                "minecraft:overworld",
                qexed_packet::net_types::Position { x: 1, y, z },
                stone_block_state(),
            );
        }
    }
    manager
        .spawn_local(EntitySpawnRequest {
            key: "path_follower".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Path".to_string(),
            display_name: "Path".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    for _ in 0..8 {
        manager
            .tick_ai(
                &players,
                &world,
                &crate::plugins::PluginManager::empty_for_tests(),
                &qexed_config::app::qexed::server::EntityRendering::default(),
                50,
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    let entity = manager.entity_by_key("path_follower").unwrap();
    assert!(
        entity.position.z.abs() > 0.2,
        "path follower should leave the blocked direct line, got {:?}",
        entity.position
    );
}

#[test]
fn entity_ai_applies_gravity_and_horizontal_collision() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Target".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 4.0,
            y: 65.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    for x in -1..=3 {
        world.place_block(
            "minecraft:overworld",
            qexed_packet::net_types::Position { x, y: 63, z: 0 },
            stone_block_state(),
        );
    }
    world.place_block(
        "minecraft:overworld",
        qexed_packet::net_types::Position { x: 1, y: 64, z: 0 },
        stone_block_state(),
    );
    manager
        .spawn_local(EntitySpawnRequest {
            key: "blocked".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 66.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
            },
            name: "Blocked".to_string(),
            display_name: "Blocked".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    for _ in 0..8 {
        manager
            .tick_ai(
                &players,
                &world,
                &crate::plugins::PluginManager::empty_for_tests(),
                &qexed_config::app::qexed::server::EntityRendering::default(),
                50,
            )
            .unwrap();
    }

    let entity = manager.entity_by_key("blocked").unwrap();
    assert!(entity.position.y < 66.0);
    assert!(entity.position.x < 0.7);
}

#[test]
fn entity_ai_auto_jump_starts_jump_when_horizontal_step_is_blocked() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let profile = qexed_packet::net_types::GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Target".to_string(),
        properties: Vec::new(),
    };
    let _session = players.join(
        profile,
        EntityPosition {
            x: 4.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    for x in -1..=4 {
        world.place_block(
            "minecraft:overworld",
            qexed_packet::net_types::Position { x, y: 63, z: 0 },
            stone_block_state(),
        );
    }
    world.place_block(
        "minecraft:overworld",
        qexed_packet::net_types::Position { x: 1, y: 64, z: 0 },
        stone_block_state(),
    );
    manager
        .spawn_local(EntitySpawnRequest {
            key: "jumper".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.65,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Jumper".to_string(),
            display_name: "Jumper".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: true,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    for _ in 0..5 {
        manager
            .tick_ai(
                &players,
                &world,
                &crate::plugins::PluginManager::empty_for_tests(),
                &qexed_config::app::qexed::server::EntityRendering::default(),
                50,
            )
            .unwrap();
    }

    let entity = manager.entity_by_key("jumper").unwrap();
    assert!(entity.position.y > 64.0);
    assert!(!entity.position.on_ground);
}

#[test]
fn entity_ai_walks_over_carpet_without_treating_it_as_full_block() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 4.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    for x in -1..=4 {
        world.place_block(
            "minecraft:overworld",
            qexed_packet::net_types::Position { x, y: 63, z: 0 },
            stone_block_state(),
        );
    }
    world.place_block(
        "minecraft:overworld",
        qexed_packet::net_types::Position { x: 1, y: 64, z: 0 },
        block_state_for_item_name("minecraft:white_carpet"),
    );
    manager
        .spawn_local(EntitySpawnRequest {
            key: "carpet_walker".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.65,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Walker".to_string(),
            display_name: "Walker".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "follow_nearest_player".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    for _ in 0..5 {
        manager
            .tick_ai(
                &players,
                &world,
                &crate::plugins::PluginManager::empty_for_tests(),
                &qexed_config::app::qexed::server::EntityRendering::default(),
                50,
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }

    let entity = manager.entity_by_key("carpet_walker").unwrap();
    assert!(entity.position.x > 1.0);
    assert!((64.0..64.2).contains(&entity.position.y));
}

#[test]
fn entity_ai_auto_jump_defaults_to_enabled_for_spawn_rules() {
    let rule = qexed_config::app::qexed::server::EntitySpawnRule {
        ai: "follow_nearest_player".to_string(),
        ..Default::default()
    };

    assert!(rule.auto_jump);
}

#[test]
fn entity_ai_handles_three_thousand_active_entities_with_bounded_tick_time() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Target".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let world = empty_world();
    for x in -64..=64 {
        for z in -64..=64 {
            world.place_block(
                "minecraft:overworld",
                qexed_packet::net_types::Position { x, y: 63, z },
                stone_block_state(),
            );
        }
    }

    let mut initial_positions = std::collections::HashMap::new();
    for index in 0..3_000 {
        let x = f64::from((index % 100) as i32 - 50) + 0.5;
        let z = f64::from((index / 100) as i32 - 15) + 0.5;
        let key = format!("stress_follower_{index}");
        initial_positions.insert(key.clone(), (x, z));
        manager
            .spawn_local(EntitySpawnRequest {
                key,
                kind: ManagedEntityKind::Entity,
                entity_type: "minecraft:zombie".to_string(),
                entity_type_id_override: None,
                dimension: "minecraft:overworld".to_string(),
                position: EntityPosition {
                    x,
                    y: 64.0,
                    z,
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: true,
                },
                name: format!("Stress {index}"),
                display_name: String::new(),
                skin_textures: String::new(),
                skin_signature: String::new(),
                data: 0,
                ai: "follow_nearest_player".to_string(),
                ai_params: Default::default(),
                auto_jump: true,
                spawn_rule: "stress".to_string(),
                custom_type: String::new(),
                look_at_players: false,
                main_hand_event: "interact".to_string(),
                off_hand_event: "interact_off_hand".to_string(),
                attack_event: "attack".to_string(),
            })
            .unwrap();
    }

    let rendering = qexed_config::app::qexed::server::EntityRendering {
        default_distance: 128.0,
        stack_threshold: usize::MAX,
        ..Default::default()
    };
    let mut max_tick = std::time::Duration::ZERO;
    let started = std::time::Instant::now();
    for _ in 0..8 {
        let tick_started = std::time::Instant::now();
        manager
            .tick_ai(
                &players,
                &world,
                &crate::plugins::PluginManager::empty_for_tests(),
                &rendering,
                50,
            )
            .unwrap();
        max_tick = max_tick.max(tick_started.elapsed());
        std::thread::sleep(std::time::Duration::from_millis(55));
    }
    let total = started.elapsed();

    assert!(
        max_tick <= std::time::Duration::from_millis(120),
        "3000 active entity AI tick exceeded budget: max={max_tick:?}, total={total:?}"
    );

    let moved = manager
        .list_for_dimension("minecraft:overworld")
        .into_iter()
        .filter(|entity| {
            let Some((initial_x, initial_z)) = initial_positions.get(&entity.key) else {
                return false;
            };
            (entity.position.x - initial_x).abs() > 0.0001
                || (entity.position.z - initial_z).abs() > 0.0001
        })
        .count();
    assert!(moved > 0, "stress entities should continue ticking");
}

#[test]
fn killed_entity_stays_until_death_animation_delay_expires() {
    let entity_ids = std::sync::Arc::new(EntityIdAllocator::new(1));
    let manager = EntityManager::from_config(
        &qexed_config::app::qexed::server::Entities::default(),
        entity_ids.clone(),
    )
    .unwrap();
    let players = crate::players::PlayerManager::new(entity_ids);
    let _session = players.join(
        qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Viewer".to_string(),
            properties: Vec::new(),
        },
        EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        "minecraft:overworld".to_string(),
        Vec::new(),
        "en_us".to_string(),
    );
    let entity = manager
        .spawn_local(EntitySpawnRequest {
            key: "death_animation_target".to_string(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id_override: None,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Target".to_string(),
            display_name: "Target".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: "none".to_string(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: "test".to_string(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        })
        .unwrap();

    let result = manager
        .damage_managed_entity(
            &players,
            &qexed_config::app::qexed::server::EntityRendering::default(),
            entity.entity_id,
            100.0,
        )
        .unwrap()
        .expect("damage result");

    assert!(result.killed);
    assert!(manager.entity_by_runtime_id(entity.entity_id).is_some());

    std::thread::sleep(std::time::Duration::from_millis(1_050));
    manager
        .tick_ai(
            &players,
            &empty_world(),
            &crate::plugins::PluginManager::empty_for_tests(),
            &qexed_config::app::qexed::server::EntityRendering::default(),
            50,
        )
        .unwrap();

    assert!(manager.entity_by_runtime_id(entity.entity_id).is_none());
}

#[test]
fn npc_spawn_packets_include_display_name_in_player_info() {
    let entity = ManagedEntity {
        key: "shop".to_string(),
        entity_id: 1,
        uuid: uuid::Uuid::new_v4(),
        kind: ManagedEntityKind::Npc,
        entity_type: "minecraft:player".to_string(),
        entity_type_id: 155,
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: "shop_001".to_string(),
        display_name: "Shop".to_string(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: String::new(),
        ai_params: Default::default(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: true,
        main_hand_event: "right_click".to_string(),
        off_hand_event: "left_click".to_string(),
        attack_event: "attack".to_string(),
    };

    let packets = entity.spawn_packets().unwrap();
    assert!(
        packets.len() >= 4,
        "NPC spawn should produce at least 4 packets, got {}",
        packets.len()
    );

    let player_info_bytes = &packets[0];
    let display_name_utf8 = "Shop".as_bytes();
    let found = player_info_bytes
        .windows(display_name_utf8.len())
        .any(|window| window == display_name_utf8);
    assert!(
        found,
        "PlayerInfoUpdate packet should contain display_name bytes."
    );
}

#[test]
fn npc_spawn_packets_parse_json_display_name_component() {
    let entity = ManagedEntity {
        key: "shop".to_string(),
        entity_id: 1,
        uuid: uuid::Uuid::new_v4(),
        kind: ManagedEntityKind::Npc,
        entity_type: "minecraft:player".to_string(),
        entity_type_id: 155,
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: "shop_001".to_string(),
        display_name: "{\"text\":\"Shop\",\"color\":\"gold\"}".to_string(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: String::new(),
        ai_params: Default::default(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: true,
        main_hand_event: "right_click".to_string(),
        off_hand_event: "left_click".to_string(),
        attack_event: "attack".to_string(),
    };

    let packets = entity.spawn_packets().unwrap();
    let player_info = decode_clientbound_packet::<PlayerInfoUpdate>(&packets[0]);
    let entry = player_info.entries.first().unwrap();
    assert_eq!(entry.profile_name, "Shop");
    assert_text_component_field(entry.display_name.as_ref().unwrap(), "text", "Shop");
    assert_text_component_field(entry.display_name.as_ref().unwrap(), "color", "gold");
}

#[test]
fn disguised_npc_spawn_packets_use_client_entity_type_without_player_info() {
    let entity = ManagedEntity {
        key: "guard".to_string(),
        entity_id: 7,
        uuid: uuid::Uuid::new_v4(),
        kind: ManagedEntityKind::Npc,
        entity_type: "minecraft:zombie".to_string(),
        entity_type_id: entity_type_id("minecraft:zombie").unwrap(),
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: "people".to_string(),
        display_name: "people".to_string(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: String::new(),
        ai_params: Default::default(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: true,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    };

    let packets = entity.spawn_packets().unwrap();

    assert_eq!(
        packets[0][0],
        qexed_protocol::to_client::play::add_entity::AddEntity::ID as u8
    );
    assert!(packets.iter().all(|packet| packet[0]
        != qexed_protocol::to_client::play::player_info_update::PlayerInfoUpdate::ID as u8));
}

#[test]
fn hologram_spawn_packets_parse_json_display_name_component() {
    let entity = ManagedEntity {
        key: "mine_tip".to_string(),
        entity_id: 3,
        uuid: uuid::Uuid::new_v4(),
        kind: ManagedEntityKind::Hologram,
        entity_type: "minecraft:text_display".to_string(),
        entity_type_id: 131,
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
        },
        name: "mine tip".to_string(),
        display_name: "{\"text\":\"Mine tip\",\"color\":\"aqua\"}".to_string(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: String::new(),
        ai_params: Default::default(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: false,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    };

    let packets = entity.spawn_packets().unwrap();
    let set_entity_data = decode_clientbound_packet::<SetEntityData>(&packets[2]);
    let text_component = set_entity_data
        .metadata
        .data
        .iter()
        .find_map(|metadata| match &metadata.data {
            Some(EntityMetadataEnum::TextComponent(component)) => Some(component),
            _ => None,
        })
        .unwrap();

    assert_text_component_field(text_component, "text", "Mine tip");
    assert_text_component_field(text_component, "color", "aqua");
}

#[test]
fn npc_spawn_packets_include_skin_textures_in_player_info() {
    let entity = ManagedEntity {
        key: "skin-npc".to_string(),
        entity_id: 2,
        uuid: uuid::Uuid::new_v4(),
        kind: ManagedEntityKind::Npc,
        entity_type: "minecraft:player".to_string(),
        entity_type_id: 155,
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: "skin_npc".to_string(),
        display_name: "Skin NPC".to_string(),
        skin_textures: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYWJjIn19fQ==".to_string(),
        skin_signature: "signed-by-mojang".to_string(),
        data: 0,
        ai: String::new(),
        ai_params: Default::default(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: false,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    };

    let packets = entity.spawn_packets().unwrap();
    let player_info_bytes = &packets[0];
    let textures = entity.skin_textures.as_bytes();
    let signature = entity.skin_signature.as_bytes();
    assert!(
        player_info_bytes
            .windows(textures.len())
            .any(|w| w == textures),
        "PlayerInfoUpdate packet should contain textures property"
    );
    assert!(
        player_info_bytes
            .windows(signature.len())
            .any(|w| w == signature),
        "PlayerInfoUpdate packet should contain textures signature"
    );
}

#[test]
fn player_npc_does_not_send_removed_skin_parts_metadata() {
    let entity = ManagedEntity {
        key: "player-npc".to_string(),
        entity_id: 2,
        uuid: uuid::Uuid::new_v4(),
        kind: ManagedEntityKind::Npc,
        entity_type: "minecraft:player".to_string(),
        entity_type_id: 155,
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: "player_npc".to_string(),
        display_name: "Player NPC".to_string(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: String::new(),
        ai_params: Default::default(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: false,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    };

    let packets = entity.spawn_packets().unwrap();
    let set_entity_data = decode_clientbound_packet::<SetEntityData>(&packets[3]);
    let removed_skin_parts_metadata = set_entity_data
        .metadata
        .data
        .iter()
        .any(|metadata| metadata.data == Some(EntityMetadataEnum::Byte(0x7f)));

    assert!(!removed_skin_parts_metadata);
}

#[test]
fn npc_profile_name_is_minecraft_safe() {
    assert_eq!(npc_profile_name(""), "NPC");
    assert_eq!(npc_profile_name("ab"), "ab");
    assert_eq!(
        npc_profile_name("Guide_0123456789012345"),
        "Guide_0123456789"
    );
}

fn decode_clientbound_packet<T>(packet: &Bytes) -> T
where
    T: Packet + Default,
{
    let mut payload = BytesMut::from(packet.as_ref());
    let packet_id = crate::connection::read_packet_id(&mut payload).unwrap();
    assert_eq!(packet_id, T::ID);
    crate::connection::decode_payload::<T>(&mut payload).unwrap()
}

fn assert_text_component_field(component: &qexed_nbt::Tag, key: &str, expected: &str) {
    let qexed_nbt::Tag::Compound(fields) = component else {
        panic!("text component should be an NBT compound, got {component:?}");
    };
    let Some(qexed_nbt::Tag::String(value)) = fields.get(key) else {
        panic!("text component should contain string field {key:?}, got {component:?}");
    };
    assert_eq!(&**value, expected);
}
