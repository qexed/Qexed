use qexed_packet::{PacketCodec, net_types::VarInt};
use std::fmt;
#[qexed_packet_macros::packet(id = 0x10)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Commands {
    pub nodes: Vec<Node>,
    pub root_index: VarInt,
}

/// 温馨警告：该结构体无法直接 substruct
/// 因为它是可选判定
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Node {
    pub flags: u8,
    pub children: Vec<VarInt>,
    pub redirect_node: Option<VarInt>,
    pub name: Option<String>,
    pub parser_id: Option<VarInt>,
    pub properties: Option<Varies>,
    pub suggestions_type: Option<String>,
}
// 定义一个 trait 来约束 Brigadier 支持的类型
pub trait BrigadierValue: PacketCodec + Clone + Default {
    const MIN: Self;
    const MAX: Self;
}

// 为支持的类型实现 BrigadierValue
impl BrigadierValue for f32 {
    const MIN: f32 = -f32::MAX;
    const MAX: f32 = f32::MAX;
}

impl BrigadierValue for f64 {
    const MIN: f64 = -f64::MAX;
    const MAX: f64 = f64::MAX;
}

impl BrigadierValue for i32 {
    const MIN: i32 = i32::MIN;
    const MAX: i32 = i32::MAX;
}

impl BrigadierValue for i64 {
    const MIN: i64 = i64::MIN;
    const MAX: i64 = i64::MAX;
}
// 为 Properties 为 "See below" 的类型定义结构体
// 这些结构体在需要时可以添加字段
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Brigadier<T> {
    pub flags: u8,
    pub min: Option<T>,
    pub max: Option<T>,
}
impl<T> PacketCodec for Brigadier<T>
where
    T: BrigadierValue,
{
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        // 写入 flags
        self.flags.serialize(w)?;

        // 如果 flags 的第0位为1，写入 min
        if self.flags & 0x01 != 0 {
            if let Some(min) = &self.min {
                min.serialize(w)?;
            } else {
                T::MIN.serialize(w)?;
            }
        }

        // 如果 flags 的第1位为1，写入 max
        if self.flags & 0x02 != 0 {
            if let Some(max) = &self.max {
                max.serialize(w)?;
            } else {
                T::MAX.serialize(w)?;
            }
        }

        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        // 读取 flags
        self.flags.deserialize(r)?;

        // 如果 flags 的第0位为1，读取 min
        if self.flags & 0x01 != 0 {
            let mut v: T = T::default();
            v.deserialize(r)?;
            self.min = Some(v)
        } else {
            self.min = None; // 使用默认值
        }

        // 如果 flags 的第1位为1，读取 max
        if self.flags & 0x02 != 0 {
            let mut v: T = T::default();
            v.deserialize(r)?;
            self.max = Some(v)
        } else {
            self.max = None; // 使用默认值
        }

        Ok(())
    }
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BrigadierString {
    // ID	Behavior Name	Notes
    // 0	SINGLE_WORD	Reads a single word
    // 1	QUOTABLE_PHRASE	If it starts with a ", keeps reading until another " (allowing escaping with \). Otherwise behaves the same as SINGLE_WORD
    // 2	GREEDY_PHRASE	Reads the rest of the content after the cursor. Quotes will not be removed.
    pub behavior: VarInt,
}

/// A selector, player name, or UUID.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftEntity {
    pub flags: u8,
}

/// Something that can join a team. Allows selectors and *.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftScoreHolder {
    pub flags: u8,
}

/// Represents a time duration.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftTime {
    pub min: i32,
}

/// An identifier or a tag name for a registry.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftResourceOrTag {
    pub registry: String,
}

/// An identifier or a tag name for a registry.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftResourceOrTagKey {
    pub registry: String,
}

/// An identifier for a registry.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftResource {
    pub registry: String,
}

/// An identifier for a registry.
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftResourceKey {
    pub registry: String,
}

/// An identifier for a registry(?).
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MinecraftResourceSelector {
    pub registry: String,
}

/// Minecraft 命令参数类型
/// 基于 brigadier 和 Minecraft 原版命令系统
#[derive(Debug, Clone, PartialEq)]
pub enum CommandArgumentType {
    /// Boolean value (true or false, case-sensitive)
    BrigadierBool,
    BrigadierFloat(Brigadier<f32>),
    BrigadierDouble(Brigadier<f64>),
    BrigadierInteger(Brigadier<i32>),
    BrigadierLong(Brigadier<i64>),
    BrigadierString(BrigadierString),
    MinecraftEntity(MinecraftEntity),
    /// A player, online or not. Can also use a selector, which may match one or more players (but not entities).
    MinecraftGameProfile,
    /// A location, represented as 3 numbers (which must be integers). May use relative locations with ~.
    MinecraftBlockPos,
    /// A column location, represented as 2 numbers (which must be integers). May use relative locations with ~.
    MinecraftColumnPos,
    /// A location, represented as 3 numbers (which may have a decimal point, but will be moved to the center of a block if none is specified). May use relative locations with ~.
    MinecraftVec3,
    /// A location, represented as 2 numbers (which may have a decimal point, but will be moved to the center of a block if none is specified). May use relative locations with ~.
    MinecraftVec2,
    /// A block state, optionally including NBT and state information.
    MinecraftBlockState,
    /// A block, or a block tag.
    MinecraftBlockPredicate,
    /// An item, optionally including NBT.
    MinecraftItemStack,
    /// An item, or an item tag.
    MinecraftItemPredicate,
    /// A chat color. One of the names from Formatting_codes#Color_codes, or reset. Case-insensitive.
    MinecraftColor,
    /// An RGB color encoded as hex, like FF0 or FF0000.
    MinecraftHexColor,
    /// A JSON text component.
    MinecraftComponent,
    /// A JSON object containing the text component styling fields.
    MinecraftStyle,
    /// A regular message, potentially including selectors.
    MinecraftMessage,
    /// An NBT value, parsed using JSON-NBT rules.
    MinecraftNbtCompoundTag,
    /// Represents a partial nbt tag, usable in data modify command.
    MinecraftNbtTag,
    /// A path within an NBT value, allowing for array and member accesses.
    MinecraftNbtPath,
    /// A scoreboard objective.
    MinecraftObjective,
    /// A single score criterion.
    MinecraftObjectiveCriteria,
    /// A scoreboard operator.
    MinecraftOperation,
    MinecraftParticle,
    ///
    MinecraftAngle,
    /// An angle, represented as 2 numbers (which may have a decimal point, but will be moved to the center of a block if none is specified). May use relative locations with ~.
    MinecraftRotation,
    /// A scoreboard display position slot. list, sidebar, belowName, and sidebar.team.${color} for all chat colors (reset is not included)
    MinecraftScoreboardSlot,
    MinecraftScoreHolder(MinecraftScoreHolder),
    /// A collection of up to 3 axes.
    MinecraftSwizzle,
    /// The name of a team. Parsed as an unquoted string.
    MinecraftTeam,
    /// A name for an inventory slot.
    MinecraftItemSlot,
    /// A name for multiple inventory slots(?).
    MinecraftItemSlots,
    /// An Identifier.
    MinecraftResourceLocation,
    /// A function.
    MinecraftFunction,
    /// The entity anchor related to the facing argument in the teleport command, is feet or eyes.
    MinecraftEntityAnchor,
    /// An integer range of values with a min and a max.
    MinecraftIntRange,
    /// A floating-point range of values with a min and a max.
    MinecraftFloatRange,
    /// Represents a dimension.
    MinecraftDimension,
    /// Represents a gamemode. (survival, creative, adventure or spectator)
    MinecraftGamemode,
    MinecraftTime(MinecraftTime),
    MinecraftResourceOrTag(MinecraftResourceOrTag),
    MinecraftResourceOrTagKey(MinecraftResourceOrTagKey),
    MinecraftResource(MinecraftResource),
    MinecraftResourceKey(MinecraftResourceKey),
    MinecraftResourceSelector(MinecraftResourceSelector),
    /// Mirror type (none, left_right or front_back)
    MinecraftTemplateMirror,
    /// Rotation type (none, clockwise_90, 180 or counterclockwise_90)
    MinecraftTemplateRotation,
    /// Post-worldgen heightmap type (motion_blocking, motion_blocking_no_leaves, ocean_floor and world_surface)
    MinecraftHeightmap,
    ///
    MinecraftLootTable,
    ///
    MinecraftLootPredicate,
    ///
    MinecraftLootModifier,
    /// An identifier in the dialog registry.
    MinecraftDialog,
    /// Represents a UUID value.
    MinecraftUuid,
}

impl CommandArgumentType {
    /// 获取参数类型的字符串标识符
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BrigadierBool => "brigadier:bool",
            Self::BrigadierFloat(_) => "brigadier:float",
            Self::BrigadierDouble(_) => "brigadier:double",
            Self::BrigadierInteger(_) => "brigadier:integer",
            Self::BrigadierLong(_) => "brigadier:long",
            Self::BrigadierString(_) => "brigadier:string",
            Self::MinecraftEntity(_) => "minecraft:entity",
            Self::MinecraftGameProfile => "minecraft:game_profile",
            Self::MinecraftBlockPos => "minecraft:block_pos",
            Self::MinecraftColumnPos => "minecraft:column_pos",
            Self::MinecraftVec3 => "minecraft:vec3",
            Self::MinecraftVec2 => "minecraft:vec2",
            Self::MinecraftBlockState => "minecraft:block_state",
            Self::MinecraftBlockPredicate => "minecraft:block_predicate",
            Self::MinecraftItemStack => "minecraft:item_stack",
            Self::MinecraftItemPredicate => "minecraft:item_predicate",
            Self::MinecraftColor => "minecraft:color",
            Self::MinecraftHexColor => "minecraft:hex_color",
            Self::MinecraftComponent => "minecraft:component",
            Self::MinecraftStyle => "minecraft:style",
            Self::MinecraftMessage => "minecraft:message",
            Self::MinecraftNbtCompoundTag => "minecraft:nbt_compound_tag",
            Self::MinecraftNbtTag => "minecraft:nbt_tag",
            Self::MinecraftNbtPath => "minecraft:nbt_path",
            Self::MinecraftObjective => "minecraft:objective",
            Self::MinecraftObjectiveCriteria => "minecraft:objective_criteria",
            Self::MinecraftOperation => "minecraft:operation",
            Self::MinecraftParticle => "minecraft:particle",
            Self::MinecraftAngle => "minecraft:angle",
            Self::MinecraftRotation => "minecraft:rotation",
            Self::MinecraftScoreboardSlot => "minecraft:scoreboard_slot",
            Self::MinecraftScoreHolder(_) => "minecraft:score_holder",
            Self::MinecraftSwizzle => "minecraft:swizzle",
            Self::MinecraftTeam => "minecraft:team",
            Self::MinecraftItemSlot => "minecraft:item_slot",
            Self::MinecraftItemSlots => "minecraft:item_slots",
            Self::MinecraftResourceLocation => "minecraft:resource_location",
            Self::MinecraftFunction => "minecraft:function",
            Self::MinecraftEntityAnchor => "minecraft:entity_anchor",
            Self::MinecraftIntRange => "minecraft:int_range",
            Self::MinecraftFloatRange => "minecraft:float_range",
            Self::MinecraftDimension => "minecraft:dimension",
            Self::MinecraftGamemode => "minecraft:gamemode",
            Self::MinecraftTime(_) => "minecraft:time",
            Self::MinecraftResourceOrTag(_) => "minecraft:resource_or_tag",
            Self::MinecraftResourceOrTagKey(_) => "minecraft:resource_or_tag_key",
            Self::MinecraftResource(_) => "minecraft:resource",
            Self::MinecraftResourceKey(_) => "minecraft:resource_key",
            Self::MinecraftResourceSelector(_) => "minecraft:resource_selector",
            Self::MinecraftTemplateMirror => "minecraft:template_mirror",
            Self::MinecraftTemplateRotation => "minecraft:template_rotation",
            Self::MinecraftHeightmap => "minecraft:heightmap",
            Self::MinecraftLootTable => "minecraft:loot_table",
            Self::MinecraftLootPredicate => "minecraft:loot_predicate",
            Self::MinecraftLootModifier => "minecraft:loot_modifier",
            Self::MinecraftDialog => "minecraft:dialog",
            Self::MinecraftUuid => "minecraft:uuid",
        }
    }

    /// 获取参数类型的数字ID
    pub fn id(&self) -> u8 {
        match self {
            Self::BrigadierBool => 0,
            Self::BrigadierFloat(_) => 1,
            Self::BrigadierDouble(_) => 2,
            Self::BrigadierInteger(_) => 3,
            Self::BrigadierLong(_) => 4,
            Self::BrigadierString(_) => 5,
            Self::MinecraftEntity(_) => 6,
            Self::MinecraftGameProfile => 7,
            Self::MinecraftBlockPos => 8,
            Self::MinecraftColumnPos => 9,
            Self::MinecraftVec3 => 10,
            Self::MinecraftVec2 => 11,
            Self::MinecraftBlockState => 12,
            Self::MinecraftBlockPredicate => 13,
            Self::MinecraftItemStack => 14,
            Self::MinecraftItemPredicate => 15,
            Self::MinecraftColor => 16,
            Self::MinecraftHexColor => 17,
            Self::MinecraftComponent => 18,
            Self::MinecraftStyle => 19,
            Self::MinecraftMessage => 20,
            Self::MinecraftNbtCompoundTag => 21,
            Self::MinecraftNbtTag => 22,
            Self::MinecraftNbtPath => 23,
            Self::MinecraftObjective => 24,
            Self::MinecraftObjectiveCriteria => 25,
            Self::MinecraftOperation => 26,
            Self::MinecraftParticle => 27,
            Self::MinecraftAngle => 28,
            Self::MinecraftRotation => 29,
            Self::MinecraftScoreboardSlot => 30,
            Self::MinecraftScoreHolder(_) => 31,
            Self::MinecraftSwizzle => 32,
            Self::MinecraftTeam => 33,
            Self::MinecraftItemSlot => 34,
            Self::MinecraftItemSlots => 35,
            Self::MinecraftResourceLocation => 36,
            Self::MinecraftFunction => 37,
            Self::MinecraftEntityAnchor => 38,
            Self::MinecraftIntRange => 39,
            Self::MinecraftFloatRange => 40,
            Self::MinecraftDimension => 41,
            Self::MinecraftGamemode => 42,
            Self::MinecraftTime(_) => 43,
            Self::MinecraftResourceOrTag(_) => 44,
            Self::MinecraftResourceOrTagKey(_) => 45,
            Self::MinecraftResource(_) => 46,
            Self::MinecraftResourceKey(_) => 47,
            Self::MinecraftResourceSelector(_) => 48,
            Self::MinecraftTemplateMirror => 49,
            Self::MinecraftTemplateRotation => 50,
            Self::MinecraftHeightmap => 51,
            Self::MinecraftLootTable => 52,
            Self::MinecraftLootPredicate => 53,
            Self::MinecraftLootModifier => 54,
            Self::MinecraftDialog => 55,
            Self::MinecraftUuid => 56,
        }
    }

    /// 从数字ID创建参数类型
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::BrigadierBool),
            1 => Some(Self::BrigadierFloat(Brigadier::<f32>::default())),
            2 => Some(Self::BrigadierDouble(Brigadier::<f64>::default())),
            3 => Some(Self::BrigadierInteger(Brigadier::<i32>::default())),
            4 => Some(Self::BrigadierLong(Brigadier::<i64>::default())),
            5 => Some(Self::BrigadierString(BrigadierString::default())),
            6 => Some(Self::MinecraftEntity(MinecraftEntity::default())),
            7 => Some(Self::MinecraftGameProfile),
            8 => Some(Self::MinecraftBlockPos),
            9 => Some(Self::MinecraftColumnPos),
            10 => Some(Self::MinecraftVec3),
            11 => Some(Self::MinecraftVec2),
            12 => Some(Self::MinecraftBlockState),
            13 => Some(Self::MinecraftBlockPredicate),
            14 => Some(Self::MinecraftItemStack),
            15 => Some(Self::MinecraftItemPredicate),
            16 => Some(Self::MinecraftColor),
            17 => Some(Self::MinecraftHexColor),
            18 => Some(Self::MinecraftComponent),
            19 => Some(Self::MinecraftStyle),
            20 => Some(Self::MinecraftMessage),
            21 => Some(Self::MinecraftNbtCompoundTag),
            22 => Some(Self::MinecraftNbtTag),
            23 => Some(Self::MinecraftNbtPath),
            24 => Some(Self::MinecraftObjective),
            25 => Some(Self::MinecraftObjectiveCriteria),
            26 => Some(Self::MinecraftOperation),
            27 => Some(Self::MinecraftParticle),
            28 => Some(Self::MinecraftAngle),
            29 => Some(Self::MinecraftRotation),
            30 => Some(Self::MinecraftScoreboardSlot),
            31 => Some(Self::MinecraftScoreHolder(MinecraftScoreHolder::default())),
            32 => Some(Self::MinecraftSwizzle),
            33 => Some(Self::MinecraftTeam),
            34 => Some(Self::MinecraftItemSlot),
            35 => Some(Self::MinecraftItemSlots),
            36 => Some(Self::MinecraftResourceLocation),
            37 => Some(Self::MinecraftFunction),
            38 => Some(Self::MinecraftEntityAnchor),
            39 => Some(Self::MinecraftIntRange),
            40 => Some(Self::MinecraftFloatRange),
            41 => Some(Self::MinecraftDimension),
            42 => Some(Self::MinecraftGamemode),
            43 => Some(Self::MinecraftTime(MinecraftTime::default())),
            44 => Some(Self::MinecraftResourceOrTag(
                MinecraftResourceOrTag::default(),
            )),
            45 => Some(Self::MinecraftResourceOrTagKey(
                MinecraftResourceOrTagKey::default(),
            )),
            46 => Some(Self::MinecraftResource(MinecraftResource::default())),
            47 => Some(Self::MinecraftResourceKey(MinecraftResourceKey::default())),
            48 => Some(Self::MinecraftResourceSelector(
                MinecraftResourceSelector::default(),
            )),
            49 => Some(Self::MinecraftTemplateMirror),
            50 => Some(Self::MinecraftTemplateRotation),
            51 => Some(Self::MinecraftHeightmap),
            52 => Some(Self::MinecraftLootTable),
            53 => Some(Self::MinecraftLootPredicate),
            54 => Some(Self::MinecraftLootModifier),
            55 => Some(Self::MinecraftDialog),
            56 => Some(Self::MinecraftUuid),
            _ => None,
        }
    }

    /// 从字符串标识符创建参数类型
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "brigadier:bool" => Some(Self::BrigadierBool),
            "brigadier:float" => Some(Self::BrigadierFloat(Brigadier::<f32>::default())),
            "brigadier:double" => Some(Self::BrigadierDouble(Brigadier::<f64>::default())),
            "brigadier:integer" => Some(Self::BrigadierInteger(Brigadier::<i32>::default())),
            "brigadier:long" => Some(Self::BrigadierLong(Brigadier::<i64>::default())),
            "brigadier:string" => Some(Self::BrigadierString(BrigadierString::default())),
            "minecraft:entity" => Some(Self::MinecraftEntity(MinecraftEntity::default())),
            "minecraft:game_profile" => Some(Self::MinecraftGameProfile),
            "minecraft:block_pos" => Some(Self::MinecraftBlockPos),
            "minecraft:column_pos" => Some(Self::MinecraftColumnPos),
            "minecraft:vec3" => Some(Self::MinecraftVec3),
            "minecraft:vec2" => Some(Self::MinecraftVec2),
            "minecraft:block_state" => Some(Self::MinecraftBlockState),
            "minecraft:block_predicate" => Some(Self::MinecraftBlockPredicate),
            "minecraft:item_stack" => Some(Self::MinecraftItemStack),
            "minecraft:item_predicate" => Some(Self::MinecraftItemPredicate),
            "minecraft:color" => Some(Self::MinecraftColor),
            "minecraft:hex_color" => Some(Self::MinecraftHexColor),
            "minecraft:component" => Some(Self::MinecraftComponent),
            "minecraft:style" => Some(Self::MinecraftStyle),
            "minecraft:message" => Some(Self::MinecraftMessage),
            "minecraft:nbt_compound_tag" => Some(Self::MinecraftNbtCompoundTag),
            "minecraft:nbt_tag" => Some(Self::MinecraftNbtTag),
            "minecraft:nbt_path" => Some(Self::MinecraftNbtPath),
            "minecraft:objective" => Some(Self::MinecraftObjective),
            "minecraft:objective_criteria" => Some(Self::MinecraftObjectiveCriteria),
            "minecraft:operation" => Some(Self::MinecraftOperation),
            "minecraft:particle" => Some(Self::MinecraftParticle),
            "minecraft:angle" => Some(Self::MinecraftAngle),
            "minecraft:rotation" => Some(Self::MinecraftRotation),
            "minecraft:scoreboard_slot" => Some(Self::MinecraftScoreboardSlot),
            "minecraft:score_holder" => {
                Some(Self::MinecraftScoreHolder(MinecraftScoreHolder::default()))
            }
            "minecraft:swizzle" => Some(Self::MinecraftSwizzle),
            "minecraft:team" => Some(Self::MinecraftTeam),
            "minecraft:item_slot" => Some(Self::MinecraftItemSlot),
            "minecraft:item_slots" => Some(Self::MinecraftItemSlots),
            "minecraft:resource_location" => Some(Self::MinecraftResourceLocation),
            "minecraft:function" => Some(Self::MinecraftFunction),
            "minecraft:entity_anchor" => Some(Self::MinecraftEntityAnchor),
            "minecraft:int_range" => Some(Self::MinecraftIntRange),
            "minecraft:float_range" => Some(Self::MinecraftFloatRange),
            "minecraft:dimension" => Some(Self::MinecraftDimension),
            "minecraft:gamemode" => Some(Self::MinecraftGamemode),
            "minecraft:time" => Some(Self::MinecraftTime(MinecraftTime::default())),
            "minecraft:resource_or_tag" => Some(Self::MinecraftResourceOrTag(
                MinecraftResourceOrTag::default(),
            )),
            "minecraft:resource_or_tag_key" => Some(Self::MinecraftResourceOrTagKey(
                MinecraftResourceOrTagKey::default(),
            )),
            "minecraft:resource" => Some(Self::MinecraftResource(MinecraftResource::default())),
            "minecraft:resource_key" => {
                Some(Self::MinecraftResourceKey(MinecraftResourceKey::default()))
            }
            "minecraft:resource_selector" => Some(Self::MinecraftResourceSelector(
                MinecraftResourceSelector::default(),
            )),
            "minecraft:template_mirror" => Some(Self::MinecraftTemplateMirror),
            "minecraft:template_rotation" => Some(Self::MinecraftTemplateRotation),
            "minecraft:heightmap" => Some(Self::MinecraftHeightmap),
            "minecraft:loot_table" => Some(Self::MinecraftLootTable),
            "minecraft:loot_predicate" => Some(Self::MinecraftLootPredicate),
            "minecraft:loot_modifier" => Some(Self::MinecraftLootModifier),
            "minecraft:dialog" => Some(Self::MinecraftDialog),
            "minecraft:uuid" => Some(Self::MinecraftUuid),
            _ => None,
        }
    }
}

impl fmt::Display for CommandArgumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// 定义 Varies 枚举，用于表示不同的参数属性
#[derive(Debug, Clone, PartialEq)]
pub enum Varies {
    BrigadierFloat(Brigadier<f32>),
    BrigadierDouble(Brigadier<f64>),
    BrigadierInteger(Brigadier<i32>),
    BrigadierLong(Brigadier<i64>),
    BrigadierString(BrigadierString),
    MinecraftEntity(MinecraftEntity),
    MinecraftScoreHolder(MinecraftScoreHolder),
    MinecraftTime(MinecraftTime),
    MinecraftResourceOrTag(MinecraftResourceOrTag),
    MinecraftResourceOrTagKey(MinecraftResourceOrTagKey),
    MinecraftResource(MinecraftResource),
    MinecraftResourceKey(MinecraftResourceKey),
    MinecraftResourceSelector(MinecraftResourceSelector),
}

impl Varies {
    /// 从 parser_id 和 PacketReader 创建 Varies
    pub fn from_parser_id(
        parser_id: u8,
        r: &mut qexed_packet::PacketReader,
    ) -> anyhow::Result<Option<Self>> {
        match parser_id {
            1 => {
                let mut brigadier_float = Brigadier::<f32>::default();
                brigadier_float.deserialize(r)?;
                Ok(Some(Varies::BrigadierFloat(brigadier_float)))
            }
            2 => {
                let mut brigadier_double = Brigadier::<f64>::default();
                brigadier_double.deserialize(r)?;
                Ok(Some(Varies::BrigadierDouble(brigadier_double)))
            }
            3 => {
                let mut brigadier_integer = Brigadier::<i32>::default();
                brigadier_integer.deserialize(r)?;
                Ok(Some(Varies::BrigadierInteger(brigadier_integer)))
            }
            4 => {
                let mut brigadier_long = Brigadier::<i64>::default();
                brigadier_long.deserialize(r)?;
                Ok(Some(Varies::BrigadierLong(brigadier_long)))
            }
            5 => {
                let mut brigadier_string = BrigadierString::default();
                brigadier_string.deserialize(r)?;
                Ok(Some(Varies::BrigadierString(brigadier_string)))
            }
            6 => {
                let mut minecraft_entity = MinecraftEntity::default();
                minecraft_entity.deserialize(r)?;
                Ok(Some(Varies::MinecraftEntity(minecraft_entity)))
            }
            31 => {
                let mut minecraft_score_holder = MinecraftScoreHolder::default();
                minecraft_score_holder.deserialize(r)?;
                Ok(Some(Varies::MinecraftScoreHolder(minecraft_score_holder)))
            }
            43 => {
                let mut minecraft_time = MinecraftTime::default();
                minecraft_time.deserialize(r)?;
                Ok(Some(Varies::MinecraftTime(minecraft_time)))
            }
            44 => {
                let mut minecraft_resource_or_tag = MinecraftResourceOrTag::default();
                minecraft_resource_or_tag.deserialize(r)?;
                Ok(Some(Varies::MinecraftResourceOrTag(
                    minecraft_resource_or_tag,
                )))
            }
            45 => {
                let mut minecraft_resource_or_tag_key = MinecraftResourceOrTagKey::default();
                minecraft_resource_or_tag_key.deserialize(r)?;
                Ok(Some(Varies::MinecraftResourceOrTagKey(
                    minecraft_resource_or_tag_key,
                )))
            }
            46 => {
                let mut minecraft_resource = MinecraftResource::default();
                minecraft_resource.deserialize(r)?;
                Ok(Some(Varies::MinecraftResource(minecraft_resource)))
            }
            47 => {
                let mut minecraft_resource_key = MinecraftResourceKey::default();
                minecraft_resource_key.deserialize(r)?;
                Ok(Some(Varies::MinecraftResourceKey(minecraft_resource_key)))
            }
            48 => {
                let mut minecraft_resource_selector = MinecraftResourceSelector::default();
                minecraft_resource_selector.deserialize(r)?;
                Ok(Some(Varies::MinecraftResourceSelector(
                    minecraft_resource_selector,
                )))
            }
            _ => Ok(None), // 其他 ID 对应 N/A 类型，没有额外属性
        }
    }

    /// 将 Varies 序列化到 PacketWriter
    pub fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        match self {
            Varies::BrigadierFloat(inner) => inner.serialize(w),
            Varies::BrigadierDouble(inner) => inner.serialize(w),
            Varies::BrigadierInteger(inner) => inner.serialize(w),
            Varies::BrigadierLong(inner) => inner.serialize(w),
            Varies::BrigadierString(inner) => inner.serialize(w),
            Varies::MinecraftEntity(inner) => inner.serialize(w),
            Varies::MinecraftScoreHolder(inner) => inner.serialize(w),
            Varies::MinecraftTime(inner) => inner.serialize(w),
            Varies::MinecraftResourceOrTag(inner) => inner.serialize(w),
            Varies::MinecraftResourceOrTagKey(inner) => inner.serialize(w),
            Varies::MinecraftResource(inner) => inner.serialize(w),
            Varies::MinecraftResourceKey(inner) => inner.serialize(w),
            Varies::MinecraftResourceSelector(inner) => inner.serialize(w),
        }
    }
}

// 为 Node 实现 PacketCodec
impl PacketCodec for Node {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        // 1. 写入 flags
        w.serialize(&self.flags)?;

        // 2. 写入 children
        w.serialize(&self.children)?;

        // 3. 如果有重定向节点，写入 redirect_node
        if self.flags & 0x08 != 0 {
            if let Some(redirect_node) = &self.redirect_node {
                w.serialize(redirect_node)?;
            } else {
                w.serialize(&VarInt(-1))?;
            }
        }
        let node_type = self.flags & 0x03;
        if node_type == 0x01 || node_type == 0x02 {
            // 4. 写入 name（总是写入）
            if let Some(name) = &self.name {
                w.serialize(name)?;
            } else {
                w.serialize(&"".to_string())?;
            }
        }

        // 5. 如果有参数，写入 parser_id
        if node_type == 0x02 {
            if let Some(parser_id) = &self.parser_id {
                w.serialize(parser_id)?;

                // 如果 parser_id 对应的类型有属性，写入 properties
                if let Some(properties) = &self.properties {
                    properties.serialize(w)?;
                }
            } else {
                w.serialize(&VarInt(0))?;
            }
        }

        // 6. 如果有建议类型，写入 suggestions_type
        if self.flags & 0x10 != 0 {
            if let Some(suggestions_type) = &self.suggestions_type {
                w.serialize(suggestions_type)?;
            } else {
                w.serialize(&"".to_string())?;
            }
        }

        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        // 1. 读取 flags
        self.flags = r.deserialize()?;

        // 2. 读取 children
        self.children = r.deserialize()?;

        // 3. 如果有重定向节点，读取 redirect_node
        if self.flags & 0x08 != 0 {
            self.redirect_node = Some(r.deserialize()?);
        } else {
            self.redirect_node = None;
        }

        // 4. 读取 name（仅对literal和argument节点）
        let node_type = self.flags & 0x03;
        if node_type == 0x01 || node_type == 0x02 { // literal或argument节点
            self.name = Some(r.deserialize()?);
        } else {
            self.name = None;
        }

        // 5. 如果有参数，读取 parser_id
        if node_type == 0x02 {
            self.parser_id = Some(r.deserialize()?);

            // 如果 parser_id 对应的类型有属性，读取 properties
            if let Some(parser_id_val) = &self.parser_id {
                let parser_id_u8 = parser_id_val.0 as u8;
                self.properties = Varies::from_parser_id(parser_id_u8, r)?;
            } else {
                self.properties = None;
            }
        } else {
            self.parser_id = None;
            self.properties = None;
        }

        // 6. 如果有建议类型，读取 suggestions_type
        if self.flags & 0x10 != 0 {
            self.suggestions_type = Some(r.deserialize()?);
        } else {
            self.suggestions_type = None;
        }

        Ok(())
    }
}
