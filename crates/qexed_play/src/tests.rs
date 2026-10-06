//! qexed_play 集成测试（v4 play/tests.rs 的 v6 适配子集）。
//!
//! v4 原文件的多数测试针对 runtime.rs（v4 play.rs 拷贝，v6 尚未迁移，
//! lib.rs 中仍注释）。本文件只保留已迁移模块（util/chunks/scoreboard/
//! chat_support/config）的测试；runtime 相关测试（方块交互/tick/活塞/
//! 作物/掉落定位/world edit 区域等）待 runtime.rs 迁移后恢复：
//! TODO(assembly)：runtime.rs 落地后从 v4 crates/qexed/src/play/tests.rs
//! 取回其余测试并按本 crate 基座改符号。

use super::chunks::{ChunkSendState, DEFAULT_PARALLELISM_FOR_TESTS, MAX_PARALLELISM_FOR_TESTS, chunk_load_parallelism_limit};
use super::config::GameMode;
use super::util::{
    acknowledged_player_ability_flags, chunk_coord, dimension_type_holder_id, keep_alive_id,
    player_ability_flags, text_component, translatable_component,
};
use qexed_protocol::to_client::play::player_abilities::PlayerAbilities;
use std::time::{Duration, Instant};

#[test]
fn chunk_coord_uses_floor_division() {
    assert_eq!(chunk_coord(0.0), 0);
    assert_eq!(chunk_coord(15.9), 0);
    assert_eq!(chunk_coord(16.0), 1);
    assert_eq!(chunk_coord(-0.1), -1);
    assert_eq!(chunk_coord(-16.0), -1);
    assert_eq!(chunk_coord(-16.1), -2);
}

#[test]
fn dimension_type_holder_id_matches_registry_order_plus_one() {
    assert_eq!(dimension_type_holder_id("minecraft:overworld"), 1);
    assert_eq!(dimension_type_holder_id("minecraft:the_nether"), 4);
}

#[test]
fn keep_alive_id_is_non_negative() {
    assert!(keep_alive_id() >= 0);
}

#[test]
fn chunk_load_parallelism_is_bounded() {
    assert_eq!(
        chunk_load_parallelism_limit(0),
        DEFAULT_PARALLELISM_FOR_TESTS
    );
    assert_eq!(chunk_load_parallelism_limit(1), 1);
    assert_eq!(chunk_load_parallelism_limit(usize::MAX), MAX_PARALLELISM_FOR_TESTS);
}

#[test]
fn player_abilities_follow_game_mode() {
    assert_eq!(player_ability_flags(GameMode::Survival, false), 0);
    assert_eq!(
        player_ability_flags(GameMode::Survival, true),
        PlayerAbilities::CAN_FLY
    );
    assert_eq!(
        player_ability_flags(GameMode::Creative, false),
        PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD
    );
    assert_eq!(
        player_ability_flags(GameMode::Spectator, false),
        PlayerAbilities::INVULNERABLE | PlayerAbilities::FLYING | PlayerAbilities::CAN_FLY
    );
}

#[test]
fn acknowledged_player_abilities_confirm_flight_only_when_allowed() {
    assert_eq!(
        acknowledged_player_ability_flags(GameMode::Survival, false, PlayerAbilities::FLYING),
        0
    );
    assert_eq!(
        acknowledged_player_ability_flags(GameMode::Survival, true, PlayerAbilities::FLYING),
        PlayerAbilities::CAN_FLY | PlayerAbilities::FLYING
    );
    assert_eq!(
        acknowledged_player_ability_flags(GameMode::Survival, true, 0),
        PlayerAbilities::CAN_FLY
    );
}

#[test]
fn translatable_component_uses_minecraft_translation_key() {
    let component = translatable_component(
        "death.fell.accident.water",
        vec![text_component("Steve")],
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
