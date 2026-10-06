//! 统计键模型。

use serde::{Deserialize, Serialize};

/// 带类型的统计（ mined/crafted/used/... + 注册表条目 id）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypedStat {
    pub kind: TypedStatKind,
    /// 条目 id（方块/物品/实体类型），custom 类忽略。
    pub entry: String,
}

/// 9 个统计类型（26.3 stat_type 注册表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedStatKind {
    Mined,
    Crafted,
    Used,
    Broken,
    Dropped,
    PickedUp,
    Killed,
    KilledBy,
    Custom,
}

impl TypedStatKind {
    /// stat_type 注册表 protocol id（AwardStats 编码用）。
    pub fn registry_id(&self) -> i32 {
        match self {
            Self::Mined => 0,
            Self::Crafted => 1,
            Self::Used => 2,
            Self::Broken => 3,
            Self::Dropped => 5,
            Self::PickedUp => 4,
            Self::Killed => 6,
            Self::KilledBy => 7,
            Self::Custom => 8,
        }
    }

    pub fn from_registry_id(id: i32) -> Option<Self> {
        Some(match id {
            0 => Self::Mined,
            1 => Self::Crafted,
            2 => Self::Used,
            3 => Self::Broken,
            4 => Self::PickedUp,
            5 => Self::Dropped,
            6 => Self::Killed,
            7 => Self::KilledBy,
            8 => Self::Custom,
            _ => return None,
        })
    }

    /// 原版 buildName 前缀（统计页数据键形态）。
    pub fn name_prefix(&self) -> &'static str {
        match self {
            Self::Mined => "minecraft:mined",
            Self::Crafted => "minecraft:crafted",
            Self::Used => "minecraft:used",
            Self::Broken => "minecraft:broken",
            Self::Dropped => "minecraft:dropped",
            Self::PickedUp => "minecraft:picked_up",
            Self::Killed => "minecraft:killed",
            Self::KilledBy => "minecraft:killed_by",
            Self::Custom => "minecraft:custom",
        }
    }
}

/// 统计键：typed 或 custom（custom 即原版 custom_stat 注册表条目）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatKey {
    /// 挖掘/合成/使用/击杀等带条目的统计。
    Typed(TypedStat),
    /// 原版自定义统计（minecraft:play_time 等 78 个）。
    Custom(&'static str),
}

impl StatKey {
    pub fn mined(block: &str) -> Self {
        Self::Typed(TypedStat { kind: TypedStatKind::Mined, entry: block.to_string() })
    }

    pub fn crafted(item: &str) -> Self {
        Self::Typed(TypedStat { kind: TypedStatKind::Crafted, entry: item.to_string() })
    }

    pub fn used(item: &str) -> Self {
        Self::Typed(TypedStat { kind: TypedStatKind::Used, entry: item.to_string() })
    }

    pub fn killed(entity: &str) -> Self {
        Self::Typed(TypedStat { kind: TypedStatKind::Killed, entry: entity.to_string() })
    }

    pub fn custom(key: &'static str) -> Self {
        Self::Custom(key)
    }

    /// 完整数据键（原版 buildName 形态："minecraft:mined:minecraft:stone"）。
    pub fn full_name(&self) -> String {
        match self {
            Self::Typed(stat) => format!("{}:{}", stat.kind.name_prefix(), stat.entry),
            Self::Custom(key) => (*key).to_string(),
        }
    }
}

/// 统计值（计数或距离类累计，统一 i64 存储）。
pub type StatValue = i64;
