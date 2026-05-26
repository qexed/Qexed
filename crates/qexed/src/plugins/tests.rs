use std::{fs, path::Path};

use super::{BlockDropQuery, MiningSpeedQuery, PluginManager, files::plugin_files};

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
