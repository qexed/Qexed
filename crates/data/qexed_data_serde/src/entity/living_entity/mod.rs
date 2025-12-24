// 使用正确的导入路径
pub mod avatar;
pub mod mob;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::entity::Entity;
use crate::entity::types::NbtUuid;
use crate::item:: Item;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivingEntity {
    #[serde(flatten)]
    pub entity: Entity,
    /// 生物的伤害吸收值。
    #[serde(rename = "AbsorptionAmount")]
    #[serde(default = "default_absorption_amount")]
    pub absorption_amount: f32,
    /// 该生物的状态效果的列表。如果当前生物没有状态效果则不存在此标签。
    #[serde(rename = "active_effects")]
    #[serde(default)]
    pub active_effects: Option<Vec<ActiveEffect>>,
    // 当前生物的属性的列表。
    #[serde(default)]
    pub attributes: Vec<Attributes>,
    // 当前生物需要记住的所有信息。(存储时必须存在)
    // #[serde(rename = "Brain")]
    // pub brain: Option<Brain>,
    /// 距离生物死亡而被删除的时间，同时控制死亡动画。生物存活时为0，濒死时每游戏刻增加1，直至20游戏刻（1秒）时生物被删除。
    #[serde(rename = "DeathTime")]
    #[serde(default)]
    pub death_time: i16,
    /// 生物装备的物品。玩家主手槽位的物品不在此处保存。
    #[serde(default)]
    pub equipment: Option<Item>,
    /// 表示生物是否处于滑翔状态。
    #[serde(rename = "FallFlying")]
    #[serde(default)]
    pub fall_flying: bool,
    /// 最大生命值
    #[serde(rename = "Health")]
    #[serde(default = "default_health")]
    pub health: f32,
    /// 生物上次被伤害的时间，以距离生物被创建的时间为准。每当生物被伤害后都被更新到最近的值，并每101刻更新一次直到攻击实体消失。可以通过/data变更，但指定的值不会影响该项自动更新，当生物受到伤害时仍会被覆盖。
    #[serde(rename = "HurtByTimestamp")]
    #[serde(default)]
    pub hurt_by_timestamp: i32,
    #[serde(rename = "HurtTime")]
    #[serde(default)]
    pub hurt_time: i16,
    /// 生物最近一次被生物攻击时的生物UUID信息，当攻击生物死亡或卸载时此值被清除。
    #[serde(default)]
    pub last_hurt_by_mob: Option<NbtUuid>,
    /// 生物最近一次被玩家攻击时的玩家UUID信息。
    #[serde(default)]
    pub last_hurt_by_player: Option<NbtUuid>,
    /// （当整型数组last_hurt_by_player存在时存在并有效）生物被玩家攻击后此值被设置为100游戏刻（5秒），每游戏刻减少1，当此值降低到0以下时清除玩家UUID信息。
    #[serde(default)]
    pub last_hurt_by_player_memory_time: Option<i32>,
    /// 生物路径点图标数据，不存在时默认为{"style":"minecraft:default"}
    #[serde(default)]
    pub locator_bar_icon: Option<LocatorBarIcon>,
    /// 当前生物正在睡觉的床的坐标，如果当前生物不在睡觉则不存在此标签。
    #[serde(default)]
    pub sleeping_pos: Option<Vec<i32>>,
    /// （不能导出和保存，仅可以加载或使用/data修改）设置当前生物所属的队伍。
    #[serde(rename = "Team")]
    #[serde(skip_serializing)]
    #[serde(default)]
    pub team: Option<String>,
    /// （当整型数组last_hurt_by_mob存在时存在并有效）生物最近一次被生物攻击后，现在距离上次攻击的游戏刻数。
    #[serde(default)]
    pub ticks_since_last_hurt_by_mob: Option<i32>,
}

// 默认值函数
fn default_absorption_amount() -> f32 {
    0.0
}

fn default_health() -> f32 {
    20.0 // 大多数生物默认生命值为20
}

impl Default for LivingEntity {
    fn default() -> Self {
        Self {
            entity: Entity::default(), // 假设 Entity 实现了 Default
            absorption_amount: default_absorption_amount(),
            active_effects: None,
            attributes: Vec::new(),
            death_time: 0,
            equipment: None,
            fall_flying: false,
            health: default_health(),
            hurt_by_timestamp: 0,
            hurt_time: 0,
            last_hurt_by_mob: None,
            last_hurt_by_player: None,
            last_hurt_by_player_memory_time: None,
            locator_bar_icon: None,
            sleeping_pos: None,
            team: None,
            ticks_since_last_hurt_by_mob: None,
        }
    }
}

/// 状态效果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveEffect {
    #[serde(flatten)]
    pub effect: HiddenEffect,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HiddenEffect {
    /// 表示状态效果是否是被信标添加的。如果不存在则为false
    #[serde(default)]
    
    pub ambient: bool,
    /// （无符号8位整数，0≤值≤255）状态效果的倍率。0表示倍率0，即等级1，以此类推。由于此数字为无符号整数，当值超过127时显示为负数但实际为正数，保存数字s和实际代表数字a的关系为a=256+s。如果不存在则为0。
    #[serde(default)]
    pub amplifier: Option<u8>,
    /// 距离状态效果失效的时间刻数。如果此值为-1，则此状态效果不会失效。如果不存在则为0。
    #[serde(default)]
    pub duration: Option<i32>,
    /// 与此状态效果类型相同，但因为等级更低而被覆盖的状态效果信息。当外层状态效果过期失效后，此层状态效果就会尝试生效。具有除字符串id外的此结构所有标签，递归定义。
    #[serde(default)]
    pub hidden_effect: Option<Box<HiddenEffect>>,
    /// 表示是否显示状态效果的图标。如果不存在则与布尔型show_particles值相同。
    #[serde(default)]
    
    pub show_icon: bool,
    /// show_particles：表示是否显示粒子效果。如果不存在则为true
    #[serde(default)]
    
    pub show_particles: bool,
}

/// 当前生物的属性的列表。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Attributes {
    /// （命名空间ID）属性的命名空间ID。
    pub id: String,
    /// 属性的基础值。
    pub base: f64,
    /// 限制本条属性的修饰符。修饰符在内部计算中调整基础值，但并不更改原值。修饰符永远不会使基础值调整后超过属性的最大值或最小值。
    #[serde(default)]
    pub modifiers: Option<Vec<Modifier>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Modifier {
    ///（命名空间ID）属性修饰符的命名空间ID。
    pub id: String,
    /// 计算中修饰符调整基础值的数值。
    pub amount: f64,
    /// 定义修饰符对属性的基础值的运算模式。可以为add_value（Op0）、add_multiplied_base（Op1）、add_multiplied_total（Op2）。如果设置值无效，则为add_value（Op0）。
    pub operation: String,
}

/// 当前生物需要记住的所有信息。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Brain {
    // /// 生物的记忆，用于生物AI的计算。
    // TODO:暂未实现,后续写AI的时候再回来实现
    // pub memories: Modifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_i32(color_int: i32) -> Self {
        Self {
            r: ((color_int >> 16) & 0xFF) as u8,
            g: ((color_int >> 8) & 0xFF) as u8,
            b: (color_int & 0xFF) as u8,
        }
    }

    pub fn to_i32(&self) -> i32 {
        ((self.r as i32) << 16) | ((self.g as i32) << 8) | (self.b as i32)
    }

    pub fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }

    pub fn to_hex(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

// 手动实现 Serialize：优先序列化为整数，但也可以支持数组
impl Serialize for RgbColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 根据描述，游戏保存为整数形式，所以我们优先序列化为整数
        serializer.serialize_i32(self.to_i32())
    }
}

// 手动实现 Deserialize：支持从数组或整数解析
impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 使用 Visitor 来处理多种可能的输入类型
        struct ColorVisitor;

        impl<'de> Visitor<'de> for ColorVisitor {
            type Value = RgbColor;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an integer or a sequence of 3 bytes")
            }

            // 处理整数格式
            fn visit_i32<E>(self, value: i32) -> Result<RgbColor, E>
            where
                E: de::Error,
            {
                Ok(RgbColor::from_i32(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<RgbColor, E>
            where
                E: de::Error,
            {
                if value < i32::MIN as i64 || value > i32::MAX as i64 {
                    return Err(E::custom("color integer out of range"));
                }
                Ok(RgbColor::from_i32(value as i32))
            }

            // 处理数组格式 [R, G, B]
            fn visit_seq<A>(self, mut seq: A) -> Result<RgbColor, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let r = seq
                    .next_element::<i8>()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?
                    as u8;
                let g = seq
                    .next_element::<i8>()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?
                    as u8;
                let b = seq
                    .next_element::<i8>()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?
                    as u8;

                // 确保没有多余的元素
                if seq.next_element::<i8>()?.is_some() {
                    return Err(de::Error::invalid_length(4, &self));
                }

                Ok(RgbColor::new(r, g, b))
            }
        }

        deserializer.deserialize_any(ColorVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocatorBarIcon {
    /// 路径点样式的命名空间ID
    #[serde(default = "default_locator_bar_icon_style")]
    pub style: String,

    /// 覆写此生物的路径点图标颜色（RGB）
    /// 支持从整数或数组格式读取，但总是序列化为整数
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub color: Option<RgbColor>,
}

fn default_locator_bar_icon_style() -> String {
    "minecraft:default".to_string()
}

impl LocatorBarIcon {
    pub fn default() -> Self {
        Self {
            style: default_locator_bar_icon_style(),
            color: None,
        }
    }
}
