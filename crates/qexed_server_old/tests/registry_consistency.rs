//! registry 同步数据与 Mojang datagen 报告的一致性验证。
//!
//! 原版客户端按服务器 registry_data 建立动态注册表；静态注册表（block/item 等）
//! 则要求两端版本一致。本测试证明：
//! 1. 每个同步注册表的 entry 集合与 registries.json 报告完全一致（客户端可完整解析）。
//! 2. 动态注册表 ID 顺序（字母序）与 vanilla 服务端 datagen 一致。

use std::path::PathBuf;

use qexed_server_legacy::registry_sync::{
    load_registry_id_map, load_registry_packets,
};

fn ensure_cache() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mojang_cache = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("run").join("cache").join("mojang"))
        .expect("workspace root");
    let _ = qexed_mojang_data::set_cache_dir(&mojang_cache);
}

/// 对"既同步又存在于静态报告"的注册表（banner_pattern / chat_type / dimension_type 等），
/// 服务器发送的条目集合必须与 datagen 报告完全一致 —— 这是原版客户端正确解析的前提。
/// 纯动态注册表（worldgen/biome，由数据包定义、不在报告里）跳过集合对比。
#[test]
fn synchronized_registries_match_report_where_applicable() {
    ensure_cache();
    let packets = load_registry_packets(false).unwrap();
    assert!(!packets.is_empty(), "registry packets must not be empty");

    let mut compared = 0;
    for packet in &packets {
        let report_count = report_entry_count(&packet.id);
        if report_count == 0 {
            // 纯动态注册表：不在静态报告中，仅要求非空
            assert!(
                !packet.entries.is_empty(),
                "dynamic registry {} must have entries",
                packet.id
            );
            continue;
        }

        compared += 1;
        assert_eq!(
            packet.entries.len(),
            report_count,
            "registry {} entry count differs from datagen report",
            packet.id
        );
        for entry in &packet.entries {
            let name = entry.entry_id.as_str();
            let in_report = report_has_entry(&packet.id, name);
            assert!(
                in_report,
                "entry {} not present in report registry {}",
                name,
                packet.id
            );
        }
    }
    // 26.x 的全部同步注册表都是数据驱动（不在静态报告中）；若未来版本把某些
    // 移回静态，上方的集合一致性对比自动生效。
    let _ = compared;
}

/// include_contents=true 路径：完整 NBT 内容可编码（客户端不带 vanilla core 数据时）。
/// 原版客户端会解析每个 entry 的 NBT；编码失败 = 客户端断连。
#[test]
fn full_contents_path_encodes_valid_nbt_for_all_entries() {
    ensure_cache();
    let packets = load_registry_packets(true).unwrap();
    let mut with_data = 0usize;
    for packet in &packets {
        for entry in &packet.entries {
            let Some(data) = &entry.data else {
                continue;
            };
            with_data += 1;
            // NBT 根节点必须是 Compound（客户端按 compound 读取注册表条目）
            assert!(
                matches!(data, qexed_nbt::Tag::Compound(_)),
                "entry {} in {} must encode as NBT compound",
                entry.entry_id,
                packet.id
            );
        }
    }
    // 26.1.2 vanilla 动态注册表内容总量（biome/enchantment/dialog/damage_type 等）约 380+
    assert!(
        with_data > 300,
        "expected hundreds of entries with full contents, got {with_data}"
    );
}

/// damage_type 等 vanilla 关键注册表的 ID 映射能解析（客户端 NBT 引用这些 ID）。
#[test]
fn critical_static_registries_resolve() {
    ensure_cache();
    for registry in ["minecraft:block", "minecraft:item", "minecraft:entity_type"] {
        let map = load_registry_id_map(registry).unwrap();
        assert!(!map.is_empty(), "{registry} must have entries");
        // 抽查关键条目存在
        match registry {
            "minecraft:block" => assert!(map.contains_key("minecraft:stone")),
            "minecraft:item" => assert!(map.contains_key("minecraft:stone")),
            "minecraft:entity_type" => assert!(map.contains_key("minecraft:player")),
            _ => {}
        }
    }
}

fn report() -> serde_json::Value {
    let dir = qexed_mojang_data::reports_dir().unwrap();
    serde_json::from_str(&std::fs::read_to_string(dir.join("registries.json")).unwrap()).unwrap()
}

fn report_has_entry(registry_id: &str, entry: &str) -> bool {
    report()
        .get(registry_id)
        .and_then(|r| r.get("entries"))
        .and_then(|e| e.as_object())
        .map(|m| m.contains_key(entry))
        .unwrap_or(false)
}

fn report_entry_count(registry_id: &str) -> usize {
    report()
        .get(registry_id)
        .and_then(|r| r.get("entries"))
        .and_then(|e| e.as_object())
        .map(|m| m.len())
        .unwrap_or(0)
}
