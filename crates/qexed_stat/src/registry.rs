//! 统计 id 解析：StatKey（类别 + 条目名）→ award_stats 线上三元组。
//!
//! 全部 id 来自 `qexed_registry::load_registry_id_map`（缓存的
//! `reports/registries.json`，键已规范化为 `minecraft:x`）。类别 id 同样取自
//! report，不在代码里写死，版本漂移时表现为「查不到 → 跳过」而不是错发。
use std::collections::HashMap;

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::award_stats::AwardStat;

use crate::error::StatError;

/// 统计类别（`minecraft:stat_type` 注册表条目）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatCategory {
    Mined,
    Crafted,
    Used,
    Broken,
    PickedUp,
    Dropped,
    Killed,
    KilledBy,
    Custom,
}

impl StatCategory {
    /// 注册表里的完整条目名。
    pub const fn name(self) -> &'static str {
        match self {
            StatCategory::Mined => "minecraft:mined",
            StatCategory::Crafted => "minecraft:crafted",
            StatCategory::Used => "minecraft:used",
            StatCategory::Broken => "minecraft:broken",
            StatCategory::PickedUp => "minecraft:picked_up",
            StatCategory::Dropped => "minecraft:dropped",
            StatCategory::Killed => "minecraft:killed",
            StatCategory::KilledBy => "minecraft:killed_by",
            StatCategory::Custom => "minecraft:custom",
        }
    }

    /// 该类别下 stat_id 所在的 backing 注册表。
    const fn backing_registry(self) -> &'static str {
        match self {
            StatCategory::Mined => "block",
            StatCategory::Killed | StatCategory::KilledBy => "entity_type",
            StatCategory::Custom => "custom_stat",
            _ => "item",
        }
    }
}

/// 一个统计键：类别 + 具体条目（如 custom + `minecraft:leave_game`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatKey {
    pub category: StatCategory,
    pub entry: String,
}

impl StatKey {
    pub fn custom(entry_name: &str) -> Self {
        Self {
            category: StatCategory::Custom,
            entry: format!("minecraft:{entry_name}"),
        }
    }
}

/// 各 backing 注册表的 protocol id 映射（一次性加载，整个会话复用）。
#[derive(Debug, Default, Clone)]
pub struct IdMaps {
    pub stat_type: HashMap<String, i32>,
    pub block: HashMap<String, i32>,
    pub item: HashMap<String, i32>,
    pub entity_type: HashMap<String, i32>,
    pub custom_stat: HashMap<String, i32>,
}

impl IdMaps {
    /// 从缓存 report 加载全部映射；任一注册表缺失即报错（统计功能整体不可用）。
    pub fn load() -> Result<Self, StatError> {
        Ok(Self {
            stat_type: qexed_registry::load_registry_id_map("minecraft:stat_type")?,
            block: qexed_registry::load_registry_id_map("minecraft:block")?,
            item: qexed_registry::load_registry_id_map("minecraft:item")?,
            entity_type: qexed_registry::load_registry_id_map("minecraft:entity_type")?,
            custom_stat: qexed_registry::load_registry_id_map("minecraft:custom_stat")?,
        })
    }

    /// StatKey → award_stats 线上三元组；任一级 id 查不到返回 None（调用方跳过）。
    pub fn award_stat(&self, key: &StatKey, value: i64) -> Option<AwardStat> {
        let category_id = *self.stat_type.get(key.category.name())?;
        let entry_id = match key.category.backing_registry() {
            "block" => self.block.get(key.entry.as_str())?,
            "item" => self.item.get(key.entry.as_str())?,
            "entity_type" => self.entity_type.get(key.entry.as_str())?,
            _ => self.custom_stat.get(key.entry.as_str())?,
        };
        // 原版用 int 存统计值； saturate 防御异常大的累计值
        Some(AwardStat {
            category_id: VarInt(category_id),
            stat_id: VarInt(*entry_id),
            value: VarInt(i32::try_from(value).unwrap_or(i32::MAX)),
        })
    }
}
