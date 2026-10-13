//! IdMaps 与 award_stats 构造：对已缓存的 registries report 验证 id 解析。
//!
//! 期望值锚定 26.1.2 缓存（cache/mojang/26.1.2/reports/registries.json）；
//! 缓存重刷后如协议 id 变化，这里会给出明确断言失败。
use qexed_stat::registry::{IdMaps, StatCategory, StatKey};

/// IdMaps 经 qexed_mojang_data 按 CWD 解析缓存；测试进程的 CWD 是 crate 目录，
/// 先切到工作区根（qexed_anvil 测试同款套路）。
fn chdir_workspace_root() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::env::set_current_dir(dir).expect("chdir to workspace root");
}

#[test]
fn leave_game_resolves_to_award_stat() {
    chdir_workspace_root();
    let maps = IdMaps::load().expect("load id maps from cache");
    let key = StatKey::custom("leave_game");
    let award = maps.award_stat(&key, 3).expect("leave_game should resolve");
    assert_eq!(award.value.0, 3);
    let custom_id = maps.stat_type["minecraft:custom"];
    assert_eq!(award.category_id.0, custom_id);
    // 26.1.2：custom_stat 里 leave_game 的 protocol_id
    assert_eq!(award.stat_id.0, 0);
}

#[test]
fn mined_stone_resolves_via_block_registry() {
    chdir_workspace_root();
    let maps = IdMaps::load().expect("load id maps from cache");
    let key = StatKey {
        category: StatCategory::Mined,
        entry: "minecraft:stone".into(),
    };
    let award = maps.award_stat(&key, 1).expect("stone should resolve");
    assert!(award.stat_id.0 >= 0);
}

#[test]
fn killed_zombie_resolves_via_entity_registry() {
    chdir_workspace_root();
    let maps = IdMaps::load().expect("load id maps from cache");
    let key = StatKey {
        category: StatCategory::Killed,
        entry: "minecraft:zombie".into(),
    };
    let award = maps.award_stat(&key, 5).expect("zombie should resolve");
    // 26.1.2：entity_type 里 zombie 的 protocol_id
    assert_eq!(award.stat_id.0, 150);
}

#[test]
fn unknown_entry_yields_none() {
    chdir_workspace_root();
    let maps = IdMaps::load().expect("load id maps from cache");
    let key = StatKey::custom("not_a_real_stat");
    assert!(maps.award_stat(&key, 1).is_none());
}
