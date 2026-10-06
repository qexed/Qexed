//! AwardStats 包构造（clientbound 0x03）。

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::award_stats::{AwardStat, AwardStats};

use crate::model::StatKey;

/// 由 (键, 值) 列表构造 AwardStats。
///
/// 编码映射：
/// - typed 统计：category = stat_type id，stat = 条目注册表 id
/// - custom 统计：category = 8 (custom)，stat = custom_stat 注册表 id
pub fn award_stats_packet(entries: &[(StatKey, i64)]) -> crate::Result<AwardStats> {
    let mut stats = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let Some(stat) = encode_stat(key, *value)? else {
            continue;
        };
        stats.push(stat);
    }
    Ok(AwardStats { stats })
}

fn encode_stat(key: &StatKey, value: i64) -> crate::Result<Option<AwardStat>> {
    let value = i32::try_from(value).unwrap_or(i32::MAX);
    match key {
        StatKey::Custom(name) => {
            let Some(id) = crate::registry::custom_stat_id(name) else {
                log::debug!("unknown custom stat: {name}");
                return Ok(None);
            };
            Ok(Some(AwardStat {
                category_id: VarInt(crate::registry::stat_type::CUSTOM),
                stat_id: VarInt(id),
                value: VarInt(value),
            }))
        }
        StatKey::Typed(stat) => {
            let registry = match stat.kind {
                crate::model::TypedStatKind::Mined => "minecraft:block",
                crate::model::TypedStatKind::Crafted | crate::model::TypedStatKind::Used => "minecraft:item",
                crate::model::TypedStatKind::Broken | crate::model::TypedStatKind::Dropped
                | crate::model::TypedStatKind::PickedUp => "minecraft:item",
                crate::model::TypedStatKind::Killed | crate::model::TypedStatKind::KilledBy => "minecraft:entity_type",
                crate::model::TypedStatKind::Custom => "minecraft:custom_stat",
            };
            let Some(id) = crate::registry::entry_protocol_id(registry, &stat.entry) else {
                log::debug!("unknown stat entry {} in {registry}", stat.entry);
                return Ok(None);
            };
            Ok(Some(AwardStat {
                category_id: VarInt(stat.kind.registry_id()),
                stat_id: VarInt(id),
                value: VarInt(value),
            }))
        }
    }
}
