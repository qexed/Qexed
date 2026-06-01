use super::{
    ChunkSendState, WorldEditKind, can_modify_world, chunk_coord, chunk_load_parallelism_limit,
    dimension_type_holder_id, keep_alive_id, login_dimension_names, player_ability_flags,
    world_write_mode,
};
use qexed_config::app::qexed::server::GameMode;
use qexed_packet::net_types::Position;
use qexed_protocol::to_client::play::add_entity::EntityPosition;
use qexed_protocol::to_client::play::player_abilities::PlayerAbilities;
use std::time::{Duration, Instant};

#[test]
fn chunk_coord_uses_floor_division() {
    assert_eq!(chunk_coord(0.0), 0);
    assert_eq!(chunk_coord(15.9), 0);
    assert_eq!(chunk_coord(16.0), 1);
    assert_eq!(chunk_coord(-0.1), -1);
    assert_eq!(chunk_coord(-16.0), -1);
}

#[test]
fn dimension_type_holder_id_matches_registry_order_plus_one() {
    assert_eq!(dimension_type_holder_id("minecraft:overworld"), 1);
    assert_eq!(dimension_type_holder_id("minecraft:the_nether"), 4);
}

#[test]
fn login_dimension_names_use_configured_worlds_without_forcing_overworld() {
    let mut world = qexed_config::app::qexed::server::World::default();
    world.default_dimension = "qexed:mine_a".to_string();
    world.dimension = String::new();
    world.worlds = vec![qexed_config::app::qexed::server::WorldStorage {
        id: "mine_a".to_string(),
        dimension: "qexed:mine_a".to_string(),
        dimension_type: "minecraft:overworld".to_string(),
        path: "worlds/mine_a".to_string(),
    }];
    world.instances.clear();

    assert_eq!(
        login_dimension_names(&world, "qexed:mine_a"),
        vec!["qexed:mine_a".to_string()]
    );
}

#[test]
fn keep_alive_id_is_non_negative() {
    assert!(keep_alive_id() >= 0);
}

#[test]
fn chunk_load_parallelism_is_bounded() {
    assert_eq!(
        chunk_load_parallelism_limit(0),
        super::DEFAULT_CHUNK_LOAD_PARALLELISM
    );
    assert_eq!(chunk_load_parallelism_limit(1), 1);
    assert_eq!(
        chunk_load_parallelism_limit(usize::MAX),
        super::MAX_CHUNK_LOAD_PARALLELISM
    );
}

#[test]
fn player_abilities_follow_game_mode() {
    assert_eq!(player_ability_flags(GameMode::Survival), 0);
    assert_eq!(
        player_ability_flags(GameMode::Creative),
        PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD
    );
    assert_eq!(
        player_ability_flags(GameMode::Spectator),
        PlayerAbilities::INVULNERABLE | PlayerAbilities::FLYING | PlayerAbilities::CAN_FLY
    );
}

#[test]
fn world_edit_rules_apply_read_only_and_spawn_protection() {
    let mut world = qexed_config::app::qexed::server::World::default();
    let spawn = qexed_packet::net_types::Position { x: 1, y: 64, z: 1 };
    let outside_spawn = qexed_packet::net_types::Position {
        x: 100,
        y: 64,
        z: 100,
    };

    assert!(!can_modify_world(&world, &spawn));
    assert!(can_modify_world(&world, &outside_spawn));

    world.spawn_protection_radius = 0;
    assert!(can_modify_world(&world, &spawn));

    world.read_only = true;
    assert!(!can_modify_world(&world, &outside_spawn));

    world.read_only = false;
    world.game_mode = GameMode::Adventure;
    assert!(!can_modify_world(&world, &outside_spawn));

    world.game_mode = GameMode::Spectator;
    assert!(!can_modify_world(&world, &outside_spawn));
}

#[test]
fn read_only_world_allows_runtime_edit_regions() {
    let temp = tempfile::tempdir().unwrap();
    let mut world = qexed_config::app::qexed::server::World {
        read_only: true,
        spawn_protection_radius: 0,
        ..Default::default()
    };
    world
        .edit_regions
        .push(qexed_config::app::qexed::server::WorldEditRegion {
            id: "mine".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 10,
            max_x: 20,
            min_y: 0,
            max_y: 80,
            min_z: -5,
            max_z: 5,
            allow_player_break: true,
            allow_player_place: false,
            allow_plugin_write: true,
            runtime_only: true,
        });
    let rules = crate::world::WorldRulesManager::from_world_config_for_tests(
        &world,
        temp.path().join("rules"),
    )
    .unwrap();
    let manager = crate::world::WorldManager::new(temp.path().join("world"))
        .with_edit_regions(&world.edit_regions);
    rules.set_read_only("minecraft:overworld", true).unwrap();

    let inside = Position { x: 12, y: 64, z: 0 };
    let outside = Position { x: 30, y: 64, z: 0 };
    let break_mode = world_write_mode(
        &manager,
        &world,
        &rules,
        "minecraft:overworld",
        &inside,
        WorldEditKind::Break,
        true,
    );
    let place_mode = world_write_mode(
        &manager,
        &world,
        &rules,
        "minecraft:overworld",
        &inside,
        WorldEditKind::Place,
        true,
    );
    let outside_mode = world_write_mode(
        &manager,
        &world,
        &rules,
        "minecraft:overworld",
        &outside,
        WorldEditKind::Break,
        true,
    );

    assert!(break_mode.allowed);
    assert!(break_mode.runtime_only);
    assert!(!place_mode.allowed);
    assert!(!outside_mode.allowed);
}

#[test]
fn destroy_timing_follows_game_mode() {
    assert!(super::should_destroy_block(GameMode::Creative, 0));
    assert!(!super::should_destroy_block(GameMode::Creative, 2));
    assert!(!super::should_destroy_block(GameMode::Survival, 0));
    assert!(super::should_destroy_block(GameMode::Survival, 2));
    assert!(!super::should_destroy_block(GameMode::Adventure, 2));
    assert!(!super::should_destroy_block(GameMode::Spectator, 0));
}

#[test]
fn placement_collision_checks_player_body() {
    let player = EntityPosition {
        x: 0.5,
        y: 64.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };

    assert!(super::player_intersects_block(
        &player,
        &Position { x: 0, y: 64, z: 0 }
    ));
    assert!(super::player_intersects_block(
        &player,
        &Position { x: 0, y: 65, z: 0 }
    ));
    assert!(!super::player_intersects_block(
        &player,
        &Position { x: 2, y: 64, z: 0 }
    ));
}

#[test]
fn offset_position_preserves_original() {
    let position = Position {
        x: -5,
        y: 64,
        z: 10,
    };

    assert_eq!(
        super::offset_position(&position, 0, 1, -2),
        Position { x: -5, y: 65, z: 8 }
    );
    assert_eq!(
        position,
        Position {
            x: -5,
            y: 64,
            z: 10
        }
    );
}

#[test]
fn stepped_block_position_uses_block_under_feet() {
    let position = EntityPosition {
        x: -0.2,
        y: 64.0,
        z: 10.9,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };

    assert_eq!(
        super::stepped_block_position(position),
        Some(Position {
            x: -1,
            y: 63,
            z: 10
        })
    );
    assert_eq!(
        super::stepped_block_position(EntityPosition {
            on_ground: false,
            ..position
        }),
        None
    );
}

#[test]
fn translatable_component_uses_minecraft_translation_key() {
    let component = super::translatable_component(
        "death.fell.accident.water",
        vec![super::text_component("Steve")],
    );

    let qexed_nbt::Tag::Compound(root) = component else {
        panic!("translation component should be a compound");
    };
    assert_eq!(
        root.get("translate"),
        Some(&qexed_nbt::Tag::String(std::sync::Arc::from(
            "death.fell.accident.water"
        )))
    );
    let Some(qexed_nbt::Tag::List(header, values)) = root.get("with") else {
        panic!("translation component should include arguments");
    };
    assert_eq!(header.tag_id, qexed_nbt::tag_id::COMPOUND);
    assert_eq!(header.length, 1);
    assert_eq!(values.len(), 1);
}

#[test]
fn missing_chunks_skips_in_flight_chunks() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 4);
    state.loading_chunks.insert((0, 0));
    state.visible_chunks.insert((1, 0));

    let missing = state.missing_chunks();

    assert!(!missing.contains(&(0, 0)));
    assert!(!missing.contains(&(1, 0)));
    assert_eq!(missing.len(), 7);
}

#[test]
fn center_chunk_is_first_pending_chunk_and_can_be_removed_before_parallel_loads() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), -5, 10, 1, 4);

    state.refresh_pending_chunks();

    assert_eq!(state.pending_chunks.front(), Some(&(-5, 10)));
    assert!(state.remove_pending_chunk((-5, 10)));
    assert!(!state.pending_chunks.contains(&(-5, 10)));
    assert_eq!(state.pending_chunks.len(), 8);
}

#[test]
fn delayed_unload_keeps_recently_left_chunks_visible() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
    state.visible_chunks.insert((-1, 0));
    state.visible_chunks.insert((0, 0));

    state.center_x = 2;
    let target = state.target_chunks();
    state.mark_delayed_unloads(target, Instant::now() + Duration::from_secs(4));

    assert!(state.visible_chunks.contains(&(-1, 0)));
    assert!(state.pending_unloads.contains_key(&(-1, 0)));
}

#[test]
fn delayed_unload_is_cancelled_when_chunk_returns_to_view() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
    state.visible_chunks.insert((-1, 0));

    state.center_x = 2;
    state.mark_delayed_unloads(
        state.target_chunks(),
        Instant::now() + Duration::from_secs(4),
    );
    state.center_x = 0;
    state.mark_delayed_unloads(
        state.target_chunks(),
        Instant::now() + Duration::from_secs(4),
    );

    assert!(!state.pending_unloads.contains_key(&(-1, 0)));
    assert!(state.visible_chunks.contains(&(-1, 0)));
}

#[test]
fn respawn_reset_requeues_chunks_even_when_old_view_was_visible() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
    for chunk in state.target_chunks() {
        state.visible_chunks.insert(chunk);
    }
    state.loading_chunks.insert((2, 0));
    state
        .pending_unloads
        .insert((-2, 0), Instant::now() + Duration::from_secs(4));

    state.reset_view(0, 0);

    assert!(state.visible_chunks.is_empty());
    assert!(state.loading_chunks.is_empty());
    assert!(state.pending_unloads.is_empty());
    assert_eq!(state.pending_chunks.len(), 9);
    assert!(state.pending_chunks.contains(&(0, 0)));
}
