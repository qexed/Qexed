use super::{
    EntityIdAllocator, EntityManager, EntitySpawnRequest, ManagedEntity, ManagedEntityKind,
    entity_type_id, packets::npc_profile_name,
};
use qexed_packet::Packet;
use qexed_protocol::to_client::play::add_entity::EntityPosition;

#[test]
fn entity_type_id_is_loaded_from_current_report() {
    assert_eq!(entity_type_id("minecraft:player").unwrap(), 155);
    assert_eq!(entity_type_id("minecraft:armor_stand").unwrap(), 5);
    assert_eq!(entity_type_id("minecraft:text_display").unwrap(), 131);
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

    manager
        .drop_item(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            position,
            crate::inventory::simple_item(1, 1),
        )
        .unwrap();

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
                dimension: "minecraft:overworld".to_string(),
                position,
                name: "Guide".to_string(),
                display_name: String::new(),
                skin_textures: String::new(),
                skin_signature: String::new(),
                data: 0,
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
fn npc_profile_name_is_minecraft_safe() {
    assert_eq!(npc_profile_name(""), "NPC");
    assert_eq!(npc_profile_name("ab"), "ab");
    assert_eq!(
        npc_profile_name("Guide_0123456789012345"),
        "Guide_0123456789"
    );
}
