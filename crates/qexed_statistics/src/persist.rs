//! 统计持久化（原版 stats/<uuid>.json 的兼容形态）。
//!
//! 原版格式：{ "stats": { "minecraft:custom": { "minecraft:play_time": 123, ... },
//! "minecraft:mined": { "minecraft:stone": 4 } }, "DataVersion": 4189 }

use serde::{Deserialize, Serialize};

use crate::model::{StatKey, TypedStat, TypedStatKind};

/// 原版存档根。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerializedStats {
    #[serde(default)]
    pub stats: SerializedCategories,
    #[serde(default, rename = "DataVersion")]
    pub data_version: i32,
}

/// 分类 → (条目 → 值)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerializedCategories {
    #[serde(default = "empty_map", rename = "minecraft:custom")]
    pub custom: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:mined")]
    pub mined: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:crafted")]
    pub crafted: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:used")]
    pub used: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:broken")]
    pub broken: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:dropped")]
    pub dropped: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:picked_up")]
    pub picked_up: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:killed")]
    pub killed: std::collections::BTreeMap<String, i64>,
    #[serde(default = "empty_map", rename = "minecraft:killed_by")]
    pub killed_by: std::collections::BTreeMap<String, i64>,
}

fn empty_map() -> std::collections::BTreeMap<String, i64> {
    std::collections::BTreeMap::new()
}

impl SerializedStats {
    /// 26.3 数据版本。
    pub const DATA_VERSION: i32 = 4189;

    pub fn new() -> Self {
        Self { stats: Default::default(), data_version: Self::DATA_VERSION }
    }

    /// 写入一键值。
    pub fn set(&mut self, key: &StatKey, value: i64) {
        match key {
            StatKey::Custom(name) => {
                self.stats.custom.insert((*name).to_string(), value);
            }
            StatKey::Typed(stat) => {
                let map = match stat.kind {
                    TypedStatKind::Mined => &mut self.stats.mined,
                    TypedStatKind::Crafted => &mut self.stats.crafted,
                    TypedStatKind::Used => &mut self.stats.used,
                    TypedStatKind::Broken => &mut self.stats.broken,
                    TypedStatKind::Dropped => &mut self.stats.dropped,
                    TypedStatKind::PickedUp => &mut self.stats.picked_up,
                    TypedStatKind::Killed => &mut self.stats.killed,
                    TypedStatKind::KilledBy => &mut self.stats.killed_by,
                    TypedStatKind::Custom => {
                        self.stats.custom.insert(stat.entry.clone(), value);
                        return;
                    }
                };
                map.insert(stat.entry.clone(), value);
            }
        }
    }

    /// 展开为 (键, 值) 列表。
    pub fn entries(&self) -> Vec<(String, i64)> {
        let mut out: Vec<(String, i64)> = self
            .stats
            .custom
            .iter()
            .map(|(name, value)| ((*name).clone(), *value))
            .collect();

        let push_typed = |out: &mut Vec<(String, i64)>, prefix: &str, map: &std::collections::BTreeMap<String, i64>| {
            for (entry, value) in map {
                out.push((format!("{prefix}:{entry}"), *value));
            }
        };
        push_typed(&mut out, "minecraft:mined", &self.stats.mined);
        push_typed(&mut out, "minecraft:crafted", &self.stats.crafted);
        push_typed(&mut out, "minecraft:used", &self.stats.used);
        push_typed(&mut out, "minecraft:broken", &self.stats.broken);
        push_typed(&mut out, "minecraft:dropped", &self.stats.dropped);
        push_typed(&mut out, "minecraft:picked_up", &self.stats.picked_up);
        push_typed(&mut out, "minecraft:killed", &self.stats.killed);
        push_typed(&mut out, "minecraft:killed_by", &self.stats.killed_by);
        out.sort();
        out
    }
}


/// 完整键名解析回 StatKey（"minecraft:mined:minecraft:stone" / "minecraft:play_time"）。
pub fn parse_full_name(name: &str) -> Option<crate::model::StatKey> {
    if let Some(id) = crate::registry::custom_stat_id(name) {
        let _ = id;
        // 静态键表里找（StatKey::Custom 需 'static）
        let key = crate::registry::ALL_CUSTOM.iter().find(|candidate| **candidate == name)?;
        return Some(crate::model::StatKey::Custom(key));
    }
    let (prefix, entry) = name.split_once(':')?;
    let kind = match prefix {
        "minecraft" => {
            // minecraft:mined:minecraft:stone → 前两段是类型
            let rest = &name["minecraft:".len()..];
            let (type_name, entry) = rest.split_once(':')?;
            let kind = match type_name {
                "mined" => crate::model::TypedStatKind::Mined,
                "crafted" => crate::model::TypedStatKind::Crafted,
                "used" => crate::model::TypedStatKind::Used,
                "broken" => crate::model::TypedStatKind::Broken,
                "dropped" => crate::model::TypedStatKind::Dropped,
                "picked_up" => crate::model::TypedStatKind::PickedUp,
                "killed" => crate::model::TypedStatKind::Killed,
                "killed_by" => crate::model::TypedStatKind::KilledBy,
                _ => return None,
            };
            return Some(crate::model::StatKey::Typed(crate::model::TypedStat {
                kind,
                entry: entry.to_string(),
            }));
        }
        _ => {
            let _ = entry;
            return None;
        }
    };
    let _ = kind;
    None
}
