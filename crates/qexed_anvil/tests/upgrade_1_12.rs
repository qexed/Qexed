//! 1.12 升级测试：真实 1.12 区块（扁平 ID）-> 1.13+ 调色板布局。

use std::path::PathBuf;

use qexed_anvil::chunk::{self};
use qexed_anvil::upgrade::{self};
use qexed_nbt::Tag;

fn fixtures_dir() -> Option<PathBuf> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join(".tmp/anvil_fixtures");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn load(name: &str) -> Option<Tag> {
    let dir = fixtures_dir()?;
    let path = dir.join(name);
    if !path.exists() { return None };
    let bytes = std::fs::read(path).unwrap();
    Some(qexed_nbt::from_slice_lossy(&bytes).unwrap().1)
}

#[test]
fn upgrades_1_12_chunk_to_palette_layout() {
    let Some(root) = load("1.12.chunk") else { return };

    let (upgraded, stats) = upgrade::upgrade_chunk_1_12(&root, 3337).unwrap();

    let fields = chunk::root_compound(&upgraded).unwrap();
    // 1.13+ 布局（Level 解包、sections 调色板化）
    assert!(fields.get("Level").is_none());
    assert!(fields.get("sections").is_some());
    assert_eq!(fields.get("DataVersion"), Some(&Tag::Int(3337)));
    // 字段名现代化
    assert!(!fields.contains_key("TileEntities"));

    // 5 个有方块数据的 section 全部升级
    assert_eq!(stats.sections_upgraded, 5);

    // 现代读取 API 可用
    let sections = chunk::all_section_blocks(&upgraded).unwrap();
    assert_eq!(sections.len(), 5);
    for section in &sections {
        assert_eq!(section.indices.len(), 4096);
        assert!(!section.palette.is_empty());
    }

    // 统计里报告未收录的 ID（fastnbt 的 1.12 fixture 来自模组世界，存在 >127 的 ID）
    eprintln!("unknown block ids: {:?}", stats.unknown_block_ids);
}

#[test]
fn flatten_state_maps_common_ids() {
    use qexed_anvil::upgrade::flatten_state;
    

    // 石头系
    assert_eq!(flatten_state(1, 0).unwrap().name, "minecraft:stone");
    assert_eq!(flatten_state(1, 1).unwrap().name, "minecraft:granite");
    // 羊毛染色
    assert_eq!(flatten_state(35, 14).unwrap().name, "minecraft:red_wool");
    // 原木轴向
    let log = flatten_state(17, 0b0100).unwrap(); // oak, dir=1 -> axis=x
    assert_eq!(log.name, "minecraft:oak_log");
    assert!(log.properties.iter().any(|(k, v)| k == "axis" && v == "x"));
    let log_z = flatten_state(17, 0b1000).unwrap(); // dir=2 -> axis=z
    assert!(log_z.properties.iter().any(|(k, v)| k == "axis" && v == "z"));
    // 火把朝向
    assert_eq!(flatten_state(50, 1).unwrap().name, "minecraft:wall_torch");
    assert_eq!(flatten_state(50, 5).unwrap().name, "minecraft:torch");
    // 雪层
    let snow = flatten_state(78, 3).unwrap();
    assert_eq!(snow.name, "minecraft:snow");
    assert!(snow.properties.iter().any(|(k, v)| k == "layers" && v == "3"));
    // 空气与未知
    assert_eq!(flatten_state(0, 0).unwrap().name, "minecraft:air");
    assert!(flatten_state(255, 15).is_none());
}

#[test]
fn upgraded_chunk_migrates_to_modern() {
    // 1.12 -> 1.13 后再走 migrate（1.13 -> 1.18+）：完整升级链
    let Some(root) = load("1.12.chunk") else { return };
    let (upgraded, _) = upgrade::upgrade_chunk_1_12(&root, 2724).unwrap(); // 1.17

    // 伪装成 1.13-1.17 布局（Sections/Palette 平铺）——upgrade 已产出 sections 容器化布局，
    // 直接断言它能被 migrate 识别为 AlreadyModern 或正确迁移
    let (final_chunk, outcome) = qexed_anvil::migrate::migrate_chunk(
        &upgraded,
        &qexed_anvil::migrate::MigrateOptions::default(),
    ).unwrap();
    assert_eq!(outcome, qexed_anvil::migrate::MigrationOutcome::AlreadyModern);

    // 最终区块可用现代 API 读方块
    let sections = chunk::all_section_blocks(&final_chunk).unwrap();
    assert_eq!(sections.len(), 5);
}
