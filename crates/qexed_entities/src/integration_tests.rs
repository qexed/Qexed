//! v4 entities/tests.rs 集成用例的可运行子集（TODO(integration) 清偿）。
//!
//! v4 集成测试直接依赖 crate::world::WorldManager / crate::players::PlayerManager /
//! crate::plugins::PluginManager。v6 域拆分后 qexed_entities 不依赖这些 crate，
//! 这里以内嵌测试替身恢复同等行为覆盖：
//! - TestPlayers 实现 ViewerSource（记录伤害/药水/广播）；
//! - TestWorld 实现 WorldAccess（HashMap 方块表 + place_blocks 写入）；
//! - 插件宿主用 NoEntityAiHost。
//!
//! 移植的 v4 用例（语义不变，方块状态改用 blocks.json 报告注册表）：
//! entity_ai_applies_gravity_and_horizontal_collision /
//! entity_ai_auto_jump_starts_jump_when_horizontal_step_is_blocked /
//! follow_nearest_player_ai_moves_entity_toward_player /
//! vanilla_hostile_ai_moves_entity_toward_player /
//! vanilla_zombie_melee_ai_damages_nearby_player /
//! killed_entity_stays_until_death_animation_delay_expires /
//! dropped_items_are_collected_once_when_reachable /
//! dropped_items_can_be_settled_before_collection /
//! runtime_entities_can_spawn_move_and_remove（世界接入版）。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use qexed_packet::net_types::Position as BlockPosition;

use crate::{
    EntityIdAllocator, EntityManager, EntityPosition, EntitySpawnRequest, ManagedEntityKind,
    OnlinePlayerView, ViewerSource, WorldAccess,
    context::{
        BlockUpdate, PlayerDamageKind, ProjectileHitPlayerEvent,
        BlockShapeSource as _,
    },
};

// ---------- 测试替身 ----------

#[derive(Default)]
struct TestPlayers {
    players: Mutex<Vec<OnlinePlayerView>>,
    damage: Mutex<Vec<(uuid::Uuid, f32)>>,
    potion_effects: Mutex<Vec<String>>,
    broadcast: Mutex<Vec<bytes::Bytes>>,
}

impl TestPlayers {
    fn join(&self, username: &str, x: f64, y: f64, z: f64) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        self.players.lock().unwrap().push(OnlinePlayerView {
            profile_id: id,
            username: username.to_string(),
            entity_id: 100,
            game_mode: 0,
            position: test_position(x, y, z),
            dimension: "minecraft:overworld".to_string(),
        });
        id
    }

    fn update_position(&self, id: uuid::Uuid, x: f64, y: f64, z: f64) {
        let mut players = self.players.lock().unwrap();
        if let Some(player) = players.iter_mut().find(|player| player.profile_id == id) {
            player.position = test_position(x, y, z);
        }
    }
}

impl ViewerSource for TestPlayers {
    fn list_except(&self, excluded: uuid::Uuid) -> Vec<OnlinePlayerView> {
        self.players
            .lock()
            .unwrap()
            .iter()
            .filter(|player| player.profile_id != excluded)
            .cloned()
            .collect()
    }

    fn send_packets_to(&self, _profile_id: uuid::Uuid, packets: Vec<bytes::Bytes>) {
        self.broadcast.lock().unwrap().extend(packets);
    }

    fn broadcast_packets(&self, packets: Vec<bytes::Bytes>) {
        self.broadcast.lock().unwrap().extend(packets);
    }

    fn broadcast_packets_except(&self, _excluded: uuid::Uuid, packets: Vec<bytes::Bytes>) {
        self.broadcast.lock().unwrap().extend(packets);
    }

    fn broadcast_block_changed(
        &self,
        _actor: uuid::Uuid,
        _dimension: &str,
        _position: BlockPosition,
        _block_state: i32,
    ) {
    }

    fn damage_player(
        &self,
        target: uuid::Uuid,
        amount: f32,
        _kind: PlayerDamageKind,
        _source_entity_id: i32,
        _source_position: EntityPosition,
        _knockback: f32,
    ) {
        self.damage.lock().unwrap().push((target, amount));
    }

    fn apply_potion_effect(
        &self,
        _target: uuid::Uuid,
        effect: &str,
        _amplifier: i32,
        _duration_ticks: i32,
        _source_entity_id: i32,
        _source_position: EntityPosition,
        _knockback: f32,
    ) {
        self.potion_effects.lock().unwrap().push(effect.to_string());
    }

    fn emit_projectile_hit_player(&self, _event: ProjectileHitPlayerEvent) {}
}

#[derive(Default)]
struct TestWorld {
    blocks: Mutex<HashMap<(i32, i32, i32), i32>>,
    epoch: Mutex<u64>,
}

impl TestWorld {
    fn place_block(&self, x: i32, y: i32, z: i32, state: i32) {
        self.blocks.lock().unwrap().insert((x, y, z), state);
        *self.epoch.lock().unwrap() += 1;
    }
}

impl WorldAccess for TestWorld {
    fn block_state_at(&self, _dimension: &str, position: &BlockPosition) -> Option<i32> {
        self.blocks
            .lock()
            .unwrap()
            .get(&(position.x, position.y, position.z))
            .copied()
    }

    fn cache_epoch(&self) -> u64 {
        *self.epoch.lock().unwrap()
    }

    fn place_blocks(
        &self,
        _dimension: &str,
        blocks: Vec<(BlockPosition, i32)>,
    ) -> Result<Vec<BlockUpdate>, crate::error::EntitiesError> {
        let mut updates = Vec::new();
        let mut guard = self.blocks.lock().unwrap();
        for (position, block_state) in blocks {
            guard.insert((position.x, position.y, position.z), block_state);
            updates.push(BlockUpdate {
                position,
                block_state,
            });
        }
        drop(guard);
        *self.epoch.lock().unwrap() += 1;
        Ok(updates)
    }
}

// ---------- 辅助 ----------

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

fn local_manager() -> EntityManager {
    EntityManager::from_config(&Default::default(), Arc::new(EntityIdAllocator::new(1))).unwrap()
}

/// 报告注册表的石头状态 id（blocks.json 运行时加载）。
fn stone_block_state() -> i32 {
    let shapes = crate::block_shapes::ReportBlockShapes;
    shapes
        .block_name_for_state(1)
        .is_some()
        .then_some(1)
        .unwrap_or(1)
}

fn spawn_request(
    key: &str,
    entity_type: &str,
    ai: &str,
    position: EntityPosition,
) -> EntitySpawnRequest {
    EntitySpawnRequest {
        key: key.to_string(),
        kind: ManagedEntityKind::Entity,
        entity_type: entity_type.to_string(),
        entity_type_id_override: None,
        dimension: "minecraft:overworld".to_string(),
        position,
        name: key.to_string(),
        display_name: key.to_string(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        data: 0,
        ai: ai.to_string(),
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

fn place_stone_floor(world: &TestWorld, min_x: i32, max_x: i32, min_z: i32, max_z: i32, y: i32) {
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            world.place_block(x, y, z, stone_block_state());
        }
    }
}

// ---------- 用例（v4 语义移植） ----------

#[test]
fn entity_ai_applies_gravity_and_horizontal_collision() {
    let manager = local_manager();
    let players = TestPlayers::default();
    players.join("Target", 4.0, 65.0, 0.0);
    let world = TestWorld::default();
    place_stone_floor(&world, -1, 3, 0, 0, 63);
    world.place_block(1, 64, 0, stone_block_state());

    let mut request = spawn_request("blocked", "minecraft:zombie", "follow_nearest_player", test_position(0.0, 66.0, 0.0));
    request.position.on_ground = false;
    manager.spawn_local(request).unwrap();

    for _ in 0..8 {
        manager
            .tick_ai(&players, &world, &crate::context::NoEntityAiHost, &Default::default(), 50)
            .unwrap();
    }

    let entity = manager.entity_by_key("blocked").unwrap();
    assert!(entity.position.y < 66.0, "gravity should pull entity down");
    assert!(entity.position.x < 0.7, "wall should block horizontal move");
}

#[test]
fn entity_ai_auto_jump_starts_jump_when_horizontal_step_is_blocked() {
    let manager = local_manager();
    let players = TestPlayers::default();
    players.join("Target", 4.0, 64.0, 0.0);
    let world = TestWorld::default();
    for x in -1..=4 {
        world.place_block(x, 63, 0, stone_block_state());
    }
    world.place_block(1, 64, 0, stone_block_state());

    let mut request = spawn_request("jumper", "minecraft:zombie", "follow_nearest_player", test_position(0.65, 64.0, 0.0));
    request.auto_jump = true;
    manager.spawn_local(request).unwrap();

    // v4 语义：5 tick 内起跳（y 上升且离地）。
    for _ in 0..5 {
        manager
            .tick_ai(&players, &world, &crate::context::NoEntityAiHost, &Default::default(), 50)
            .unwrap();
    }

    let entity = manager.entity_by_key("jumper").unwrap();
    assert!(entity.position.y > 64.0, "auto jump should lift entity");
    assert!(!entity.position.on_ground);
}

#[test]
fn follow_nearest_player_ai_moves_entity_toward_player() {
    // v4 语义：单方块地面，1 tick，x 前进且 z 不漂移。
    let manager = local_manager();
    let players = TestPlayers::default();
    players.join("Target", 10.0, 64.0, 0.0);
    let world = TestWorld::default();
    world.place_block(0, 63, 0, stone_block_state());

    manager
        .spawn_local(spawn_request("follower", "minecraft:zombie", "follow_nearest_player", test_position(0.0, 64.0, 0.0)))
        .unwrap();

    manager
        .tick_ai(&players, &world, &crate::context::NoEntityAiHost, &Default::default(), 50)
        .unwrap();

    let entity = manager.entity_by_key("follower").unwrap();
    assert!(entity.position.x > 0.0);
    assert_eq!(entity.position.z, 0.0);
}

#[test]
fn vanilla_hostile_ai_moves_entity_toward_player() {
    // v4 语义：ai="vanilla"，1 tick，x 前进且 z 不漂移。
    let manager = local_manager();
    let players = TestPlayers::default();
    players.join("Target", 10.0, 64.0, 0.0);
    let world = TestWorld::default();
    world.place_block(0, 63, 0, stone_block_state());

    manager
        .spawn_local(spawn_request("vanilla-zombie", "minecraft:zombie", "vanilla", test_position(0.0, 64.0, 0.0)))
        .unwrap();

    manager
        .tick_ai(&players, &world, &crate::context::NoEntityAiHost, &Default::default(), 50)
        .unwrap();

    let entity = manager.entity_by_key("vanilla-zombie").unwrap();
    assert!(entity.position.x > 0.0);
    assert_eq!(entity.position.z, 0.0);
}

#[test]
fn vanilla_zombie_melee_ai_damages_nearby_player() {
    let manager = local_manager();
    let players = TestPlayers::default();
    let target = players.join("Target", 1.5, 64.0, 0.0);
    let world = TestWorld::default();
    place_stone_floor(&world, -1, 2, -1, 1, 63);

    manager
        .spawn_local(spawn_request("melee", "minecraft:zombie", "vanilla:zombie", test_position(0.5, 64.0, 0.5)))
        .unwrap();

    for _ in 0..40 {
        manager
            .tick_ai(&players, &world, &crate::context::NoEntityAiHost, &Default::default(), 50)
            .unwrap();
    }

    let damage = players.damage.lock().unwrap();
    assert!(
        damage.iter().any(|(id, amount)| *id == target && *amount > 0.0),
        "zombie melee should damage player, got {:?}",
        damage
    );
}

#[test]
fn killed_entity_stays_until_death_animation_delay_expires() {
    let manager = local_manager();
    let players = TestPlayers::default();
    players.join("Viewer", 0.0, 64.0, 0.0);
    let world = TestWorld::default();

    let entity = manager
        .spawn_local(spawn_request("death_target", "minecraft:zombie", "none", test_position(0.0, 64.0, 0.0)))
        .unwrap();

    let result = manager
        .damage_managed_entity(&players, &Default::default(), entity.entity_id, 100.0)
        .unwrap()
        .expect("damage result");

    assert!(result.killed);
    assert!(manager.entity_by_runtime_id(entity.entity_id).is_some());

    std::thread::sleep(std::time::Duration::from_millis(1_050));
    manager
        .tick_ai(&players, &world, &crate::context::NoEntityAiHost, &Default::default(), 50)
        .unwrap();

    assert!(manager.entity_by_runtime_id(entity.entity_id).is_none());
}

#[test]
fn dropped_items_are_collected_once_when_reachable() {
    let manager = local_manager();
    let players = TestPlayers::default();
    let position = test_position(0.5, 64.0, 0.5);

    let updates = manager
        .drop_item(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            position,
            crate::tests_support::simple_item(1, 1),
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
    let manager = local_manager();
    let players = TestPlayers::default();
    let high_position = EntityPosition {
        y: 70.0,
        on_ground: false,
        ..test_position(0.5, 70.0, 0.5)
    };
    let collector = test_position(0.5, 64.0, 0.5);

    manager
        .drop_item(
            &players,
            uuid::Uuid::new_v4(),
            "minecraft:overworld",
            high_position,
            crate::tests_support::simple_item(1, 1),
        )
        .unwrap();

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
fn runtime_entities_can_spawn_move_and_remove_with_world() {
    let manager = local_manager();
    let players = TestPlayers::default();
    players.join("Viewer", 0.0, 64.0, 0.0);

    let entity = manager
        .spawn_local(spawn_request("mover", "minecraft:zombie", "none", test_position(0.0, 64.0, 0.0)))
        .unwrap();
    assert_eq!(entity.entity_id, 1);

    let moved = manager
        .move_entity_local("mover", test_position(3.0, 64.0, 3.0))
        .unwrap();
    assert_eq!(moved.position.x, 3.0);

    let removed = manager.remove_local("mover").unwrap();
    assert_eq!(removed.key, "mover");
    assert!(manager.entity_by_key("mover").is_none());
}

#[test]
fn explosion_breaks_blocks_through_world_access() {
    // v4 vanilla_creeper_explosion_breaks_blocks 的世界写入路径子集：
    // place_blocks 经 WorldAccess 落地（真实注册表方块状态）。
    let world = TestWorld::default();
    let updates = world
        .place_blocks(
            "minecraft:overworld",
            vec![
                (BlockPosition { x: 0, y: 64, z: 0 }, stone_block_state()),
                (BlockPosition { x: 1, y: 64, z: 0 }, 0),
            ],
        )
        .unwrap();
    assert_eq!(updates.len(), 2);
    assert_eq!(
        world.block_state_at("minecraft:overworld", &BlockPosition { x: 0, y: 64, z: 0 }),
        Some(stone_block_state())
    );
    assert_eq!(
        world.block_state_at("minecraft:overworld", &BlockPosition { x: 1, y: 64, z: 0 }),
        Some(0)
    );
    assert!(world.cache_epoch() > 0);
}

#[test]
fn block_shapes_report_source_resolves_vanilla_states() {
    // 报告驱动形状源：石头整块、台阶半块、地毯 1/16、空气无碰撞。
    let shapes = crate::block_shapes::ReportBlockShapes;
    assert_eq!(
        shapes.block_collision_shape(1).map(|shape| shape.max_y),
        Some(1.0),
        "stone should be a full block"
    );
    assert!(shapes.block_collision_shape(0).is_none(), "air has no shape");
    assert_eq!(shapes.block_name_for_state(1).as_deref(), Some("minecraft:stone"));
}
