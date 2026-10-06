//! v4 entities/tests.rs 的可迁移子集。
//!
//! v4 全量集成测试依赖 `PlayerManager`/`WorldManager`/`PluginManager`（v6 邻域
//! crate 尚为空壳）。这里保留：
//! - 纯注册表/模型/数据包测试（v4 qexed_entity packets.rs 的单测 + tests.rs 前段）；
//! - manager 的本地 spawn/duplicate/remove/掉落物合并等无世界依赖用例；
//! - AI 内核纯函数测试（来自 v4 manager.rs 内联 tests）。
//! TODO(integration): world/play/player 域落地后补齐 60+ 集成用例（见 done-entities.txt 清单）。

use std::{collections::BTreeMap, sync::Arc};

use bytes::Bytes;
use qexed_packet::{Packet, PacketCodec, net_types::VarInt};
use qexed_protocol::{
    to_client::play::{entity_position_sync::EntityPositionSync, set_entity_data::SetEntityData, set_entity_motion::SetEntityMotion},
    types::{EntityMetadataEnum, Slot},
};

use crate::{
    EntityIdAllocator, EntityManager, EntityPosition, EntitySpawnRequest, ManagedEntity,
    ManagedEntityKind, registry::entity_type_id,
};

fn test_entity(key: &str, entity_type: &str, kind: ManagedEntityKind) -> ManagedEntity {
    ManagedEntity {
        key: key.to_string(),
        entity_id: 7,
        uuid: uuid::Uuid::new_v4(),
        kind,
        entity_type: entity_type.to_string(),
        entity_type_id: entity_type_id(entity_type).unwrap_or(1),
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 1.0,
            y: 64.0,
            z: 2.0,
            yaw: 90.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: "Test".to_string(),
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
    }
}

fn decode_packet_id(packet: &Bytes) -> i32 {
    let mut payload = packet.clone();
    let mut reader = qexed_packet::PacketReader::new(&mut payload);
    let mut packet_id = VarInt::default();
    packet_id.deserialize(&mut reader).unwrap();
    packet_id.0
}

fn simple_item(item_id: i32, count: i32) -> Slot {
    Slot {
        item_count: VarInt(count),
        item_id: Some(VarInt(item_id)),
        ..Default::default()
    }
}

#[test]
fn entity_type_id_is_loaded_from_current_report() {
    assert_eq!(entity_type_id("minecraft:player").unwrap(), 155);
    assert_eq!(entity_type_id("minecraft:armor_stand").unwrap(), 5);
    assert_eq!(entity_type_id("minecraft:item").unwrap(), 71);
    assert_eq!(entity_type_id("minecraft:villager").unwrap(), 139);
}

#[test]
fn entity_type_id_rejects_unknown_type() {
    assert!(entity_type_id("minecraft:definitely_not_real").is_err());
}

#[test]
fn stable_entity_uuid_is_deterministic() {
    let a = crate::model_helpers::stable_entity_uuid("spawn-guide");
    let b = crate::model_helpers::stable_entity_uuid("spawn-guide");
    assert_eq!(a, b);
    assert_ne!(a, crate::model_helpers::stable_entity_uuid("other"));
}

#[test]
fn ordinary_entity_spawn_includes_display_name_metadata() {
    let mut entity = test_entity("zombie", "minecraft:zombie", ManagedEntityKind::Entity);
    entity.display_name = "Cluster Zombie".to_string();

    let packets = entity.spawn_packets().unwrap();
    let packet_ids: Vec<i32> = packets.iter().map(decode_packet_id).collect();
    assert_eq!(
        packet_ids,
        vec![
            qexed_protocol::to_client::play::add_entity::AddEntity::ID,
            qexed_protocol::to_client::play::rotate_head::RotateHead::ID,
            SetEntityData::ID,
        ]
    );

    let mut payload = packets[2].clone();
    let mut reader = qexed_packet::PacketReader::new(&mut payload);
    let mut packet_id = VarInt::default();
    packet_id.deserialize(&mut reader).unwrap();
    let mut metadata = SetEntityData::default();
    metadata.deserialize(&mut reader).unwrap();

    assert_eq!(packet_id.0, SetEntityData::ID);
    assert!(metadata.metadata.data.iter().any(|entry| {
        entry.index == 2
            && matches!(
                &entry.data,
                Some(EntityMetadataEnum::OptionTextComponent(Some(_)))
            )
    }));
    assert_eq!(
        metadata
            .metadata
            .data
            .iter()
            .find(|entry| entry.index == 3)
            .and_then(|entry| match &entry.data {
                Some(EntityMetadataEnum::Boolean(visible)) => Some(*visible),
                _ => None,
            }),
        Some(true)
    );
}

#[test]
fn slime_spawn_includes_size_metadata() {
    let mut entity = test_entity("slime", "minecraft:slime", ManagedEntityKind::Entity);
    entity.data = 2;

    let packets = entity.spawn_packets().unwrap();
    let mut payload = packets[2].clone();
    let mut reader = qexed_packet::PacketReader::new(&mut payload);
    let mut packet_id = VarInt::default();
    packet_id.deserialize(&mut reader).unwrap();
    let mut metadata = SetEntityData::default();
    metadata.deserialize(&mut reader).unwrap();

    assert_eq!(packet_id.0, SetEntityData::ID);
    assert_eq!(
        metadata
            .metadata
            .data
            .iter()
            .find(|entry| entry.index == 16)
            .and_then(|entry| match &entry.data {
                Some(EntityMetadataEnum::VarInt(size)) => Some(size.0),
                _ => None,
            }),
        Some(2)
    );
}

#[test]
fn position_packets_use_position_path_encoding() {
    let entity = test_entity("zombie", "minecraft:zombie", ManagedEntityKind::Entity);
    let packets = entity
        .position_packets_with_velocity(0.12, 0.0, -0.04)
        .unwrap();
    let mut payload = packets[0].clone();
    let mut reader = qexed_packet::PacketReader::new(&mut payload);
    let mut packet_id = VarInt::default();
    packet_id.deserialize(&mut reader).unwrap();
    let mut sync = EntityPositionSync::default();
    sync.deserialize(&mut reader).unwrap();

    assert_eq!(packet_id.0, EntityPositionSync::ID);
    assert_eq!(sync.id, VarInt(7));
    assert_eq!(sync.y_rot, 90.0);
    // motion 数据包在非零速度时应存在
    assert_eq!(decode_packet_id(&packets[1]), SetEntityMotion::ID);
}

#[test]
fn npc_profile_name_trims_and_limits() {
    use crate::npc_profile_name;
    assert_eq!(npc_profile_name("  Guide  "), "Guide");
    assert_eq!(npc_profile_name("").len(), 3);
    assert!(npc_profile_name("very long npc name").chars().count() <= 16);
}

// ---------- manager 本地（无世界）用例 ----------

fn local_manager() -> EntityManager {
    EntityManager::from_config(&Default::default(), Arc::new(EntityIdAllocator::new(1))).unwrap()
}

fn spawn_request(key: &str, entity_type: &str) -> EntitySpawnRequest {
    EntitySpawnRequest {
        key: key.to_string(),
        kind: ManagedEntityKind::Entity,
        entity_type: entity_type.to_string(),
        entity_type_id_override: None,
        dimension: "minecraft:overworld".to_string(),
        position: EntityPosition {
            x: 0.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        name: key.to_string(),
        display_name: String::new(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: "none".to_string(),
        ai_params: BTreeMap::new(),
        auto_jump: false,
        spawn_rule: String::new(),
        custom_type: String::new(),
        look_at_players: false,
        main_hand_event: "interact".to_string(),
        off_hand_event: "interact_off_hand".to_string(),
        attack_event: "attack".to_string(),
    }
}

#[test]
fn runtime_entities_can_spawn_and_remove_locally() {
    let manager = local_manager();
    let entity = manager.spawn_local(spawn_request("guard", "minecraft:zombie")).unwrap();
    assert_eq!(entity.entity_id, 1);
    assert_eq!(entity.entity_type, "minecraft:zombie");
    assert!(manager.entity_by_key("guard").is_some());

    let duplicate = manager.spawn_local(spawn_request("guard", "minecraft:zombie"));
    assert!(duplicate.is_err());

    let removed = manager.remove_local("guard").unwrap();
    assert_eq!(removed.key, "guard");
    assert!(manager.entity_by_key("guard").is_none());
}

#[test]
fn spawn_local_rejects_empty_key() {
    let manager = local_manager();
    let result = manager.spawn_local(spawn_request("  ", "minecraft:zombie"));
    assert!(matches!(
        result.unwrap_err(),
        crate::error::EntitiesError::EmptyEntityId
    ));
}

#[test]
fn dropped_items_merge_until_stack_limit() {
    let manager = local_manager();
    let position = EntityPosition {
        x: 0.5,
        y: 64.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };
    let updates = manager
        .drop_item_local_with_limits("minecraft:overworld", position, simple_item(1, 40), 2.0, 64)
        .unwrap();
    assert_eq!(updates.len(), 1);

    // 40 + 24 = 64 合并满栈（Merged），剩余 6 生成新掉落（Spawned）
    let updates = manager
        .drop_item_local_with_limits("minecraft:overworld", position, simple_item(1, 30), 2.0, 64)
        .unwrap();
    assert_eq!(updates.len(), 2);
    match &updates[0] {
        crate::manager::DroppedItemUpdate::Merged(item) => assert_eq!(item.item.item_count.0, 64),
        other => panic!("expected merge, got {other:?}"),
    }
    match &updates[1] {
        crate::manager::DroppedItemUpdate::Spawned(item) => assert_eq!(item.item.item_count.0, 6),
        other => panic!("expected spawn, got {other:?}"),
    }
}

// ---------- AI 内核纯函数（v4 manager.rs 内联 tests 迁移） ----------

#[test]
fn normalize_uuid_accepts_dashed_and_compact() {
    use crate::manager::normalize_uuid;
    assert_eq!(
        normalize_uuid("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
        Some("069a79f444e94726a5befca90e38aaf5".to_string())
    );
    assert_eq!(
        normalize_uuid("069a79f444e94726a5befca90e38aaf5"),
        Some("069a79f444e94726a5befca90e38aaf5".to_string())
    );
    assert_eq!(normalize_uuid(""), None);
    assert_eq!(normalize_uuid("not-a-uuid"), None);
}

#[test]
fn ai_kind_parses_known_kinds() {
    use crate::manager::ai_kind;
    use crate::manager::EntityAiKind;
    assert_eq!(ai_kind(""), EntityAiKind::None);
    assert_eq!(ai_kind("none"), EntityAiKind::None);
    assert_eq!(ai_kind("wander"), EntityAiKind::RandomStroll);
    assert_eq!(ai_kind("look_at_players"), EntityAiKind::LookAtPlayer);
    assert_eq!(ai_kind("follow_player"), EntityAiKind::FollowNearestPlayer);
    assert_eq!(ai_kind("vanilla:zombie"), EntityAiKind::Vanilla);
    assert_eq!(ai_kind("vanilla_projectile:arrow"), EntityAiKind::Projectile);
    assert_eq!(ai_kind("plugin:custom"), EntityAiKind::Plugin);
}

#[test]
fn default_entity_health_matches_vanilla_values() {
    use crate::manager::default_entity_health;
    assert_eq!(default_entity_health("minecraft:warden"), 500.0);
    assert_eq!(default_entity_health("minecraft:wither"), 300.0);
    assert_eq!(default_entity_health("minecraft:iron_golem"), 100.0);
    assert_eq!(default_entity_health("minecraft:zombie"), 20.0);
    assert_eq!(default_entity_health("minecraft:enderman"), 40.0);
    assert_eq!(default_entity_health("minecraft:unknown"), 20.0);
}

#[test]
fn segment_intersects_aabb_axis_tests() {
    use crate::manager::segment_intersects_aabb;
    assert!(segment_intersects_aabb(
        (0.0, 0.0, 0.0),
        (10.0, 0.0, 0.0),
        (5.0, -1.0, -1.0),
        (6.0, 1.0, 1.0)
    ));
    assert!(!segment_intersects_aabb(
        (0.0, 5.0, 0.0),
        (10.0, 5.0, 0.0),
        (5.0, -1.0, -1.0),
        (6.0, 1.0, 1.0)
    ));
}

#[test]
fn potion_effect_kind_maps_vanilla_names() {
    use crate::manager::potion_effect_kind;
    use crate::manager::EntityPotionEffectKind;
    assert_eq!(
        potion_effect_kind("minecraft:instant_damage"),
        Some(EntityPotionEffectKind::InstantDamage)
    );
    assert_eq!(
        potion_effect_kind("slowness"),
        Some(EntityPotionEffectKind::Slowness)
    );
    assert_eq!(potion_effect_kind("minecraft:unknown"), None);
}
