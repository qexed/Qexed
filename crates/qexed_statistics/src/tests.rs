//! 统计域测试。

use crate::counter::StatsCounter;
use crate::model::{StatKey, TypedStatKind};
use crate::packet::award_stats_packet;
use crate::persist::SerializedStats;
use crate::registry::{ALL_CUSTOM, CUSTOM_PLAY_TIME, custom_stat_id};

#[test]
fn counter_add_and_pending() {
    let counter = StatsCounter::shared();
    assert_eq!(counter.increment(StatKey::mined("minecraft:stone")), 1);
    assert_eq!(counter.increment(StatKey::mined("minecraft:stone")), 2);
    counter.add(StatKey::custom(CUSTOM_PLAY_TIME), 100);

    let pending = counter.take_pending();
    assert_eq!(pending.len(), 2);
    assert!(counter.take_pending().is_empty());
    assert_eq!(counter.get(&StatKey::mined("minecraft:stone")), 2);
}

#[test]
fn custom_registry_covers_play_time() {
    assert!(ALL_CUSTOM.contains(&CUSTOM_PLAY_TIME));
    assert!(custom_stat_id(CUSTOM_PLAY_TIME).is_some());
}

#[test]
fn packet_encoding_skips_unknown_entries() {
    let key = StatKey::Typed(crate::model::TypedStat {
        kind: TypedStatKind::Mined,
        entry: "minecraft:not_a_block".to_string(),
    });
    let packet = award_stats_packet(&[(key, 3)]).expect("encode");
    assert!(packet.stats.is_empty());
}

#[test]
fn packet_encoding_known_custom() {
    let packet = award_stats_packet(&[(StatKey::custom(CUSTOM_PLAY_TIME), 42)]).expect("encode");
    assert_eq!(packet.stats.len(), 1);
    assert_eq!(packet.stats[0].value.0, 42);
}

#[test]
fn persist_roundtrip() {
    let mut stats = SerializedStats::new();
    stats.set(&StatKey::mined("minecraft:stone"), 7);
    stats.set(&StatKey::custom(CUSTOM_PLAY_TIME), 1234);
    let json = serde_json::to_string(&stats).unwrap();
    let back: SerializedStats = serde_json::from_str(&json).unwrap();
    let entries = back.entries();
    assert!(entries.contains(&("minecraft:mined:minecraft:stone".to_string(), 7)));
    assert!(entries.contains(&(CUSTOM_PLAY_TIME.to_string(), 1234)));
}

#[test]
fn persist_matches_vanilla_shape() {
    // 原版 JSON 顶层键形态
    let stats = SerializedStats::new();
    let json = serde_json::to_string(&stats).unwrap();
    assert!(json.contains("\"stats\""));
    assert!(json.contains("\"DataVersion\""));
}

#[test]
fn stat_key_full_name() {
    assert_eq!(
        StatKey::mined("minecraft:stone").full_name(),
        "minecraft:mined:minecraft:stone"
    );
    assert_eq!(StatKey::custom(CUSTOM_PLAY_TIME).full_name(), "minecraft:play_time");
}
