use std::{fs, path::Path};

use super::{BlockDropQuery, MiningSpeedQuery, PluginManager, files::plugin_files};
use qexed_protocol::to_client::play::add_entity::EntityPosition;

#[test]
fn plugin_files_only_keeps_wasm_files_in_stable_order() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path().join("b.wasm"));
    touch(dir.path().join("a.txt"));
    touch(dir.path().join("a.wasm"));

    let names = plugin_files(dir.path())
        .into_iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(names, ["a.wasm", "b.wasm"]);
}

fn touch(path: impl AsRef<Path>) {
    fs::write(path, []).unwrap();
}

#[test]
fn empty_plugin_manager_keeps_mining_speed_unchanged() {
    let manager = PluginManager::empty_for_tests();
    let speed = manager.apply_mining_speed(MiningSpeedQuery {
        block_state: 1,
        block_name: "minecraft:stone".to_string(),
        item_id: None,
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
        speed: 3.0,
    });

    assert_eq!(speed, 3.0);
}

#[test]
fn empty_plugin_manager_keeps_block_drops_default() {
    let manager = PluginManager::empty_for_tests();
    let drops = manager.apply_block_drops(BlockDropQuery {
        block_state: 1,
        block_name: "minecraft:stone".to_string(),
        position: super::BlockDropPosition { x: 0, y: 64, z: 0 },
        tool_item_id: None,
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
        default_item_id: Some(1),
    });

    assert!(drops.is_none());
}

#[test]
fn empty_plugin_manager_allows_item_pickup() {
    let manager = PluginManager::empty_for_tests();
    let player = crate::players::OnlinePlayer {
        profile: qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Steve".to_string(),
            properties: Vec::new(),
        },
        entity_id: 1,
        position: EntityPosition {
            x: 0.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        },
        dimension: "minecraft:overworld".to_string(),
        equipment: Vec::new(),
        language: "zh-CN".to_string(),
    };
    let item = crate::entities::DroppedItemEntity {
        entity_id: 2,
        uuid: uuid::Uuid::new_v4(),
        dimension: "minecraft:overworld".to_string(),
        position: player.position,
        item: crate::inventory::simple_item(1, 1),
        pickup_ready_at: std::time::Instant::now(),
    };

    let response = manager.handle_player_item_pickup(&player, &item);

    assert!(!response.cancel);
    assert!(response.actions.is_empty());
}
