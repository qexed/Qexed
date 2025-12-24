use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};

use crate::{entity::{living_entity::avatar::Avatar, types}, item::Item};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    #[serde(flatten)]
    pub avatar: Avatar,
    /// 保存此存档基础数据存储文件的游戏的数据版本。如果此项不存在则游戏认为此项是-1。
    #[serde(rename = "DataVersion")]
    #[serde(default = "default_data_version")]
    pub data_version: i32,
    /// 玩家拥有的能力。
    #[serde(default)]
    pub abilities: Abilities,

    /// 玩家被任何爆炸击退时，此数据才存在，表示被爆炸击退时的坐标。当玩家落地、陷入方块、切换为创造模式、使用末影珍珠和紫颂果传送时此值被清除。
    #[serde(default)]
    pub current_explosion_impact_pos: Option<CurrentExplosionImpactPos>,
    /// 爆炸击退减少摔落伤害的最长时间，按游戏刻计。玩家被风弹物品风弹所产生的风爆击退或使用重锤时此值被设置为40游戏刻（2秒），被其他爆炸波及、或NBT列表/JSON数组current_explosion_impact_pos被清除时设置为0。此值达到0时清除NBT列表/JSON数组current_explosion_impact_pos和布尔型ignore_fall_damage_from_current_explosion。
    #[serde(default)]
    pub current_impulse_context_reset_grace_time: i32,
    #[serde(rename = "Dimension")]
    #[serde(default)]
    pub dimension: Option<String>,
    /// 与玩家绑定的末影珍珠数据。如果玩家没有绑定的末影珍珠，则此项不存在。
    #[serde(default)]
    pub ender_pearls: Option<Vec<EnderPearls>>,
    // /// 玩家末影箱里的物品。末影箱中一共有27个槽位，超出槽位范围的物品不会被加载。
    #[serde(rename = "EnderItems")]
    pub ender_items:Vec<Item>,
    pub entered_nether_pos:Vec<f64>,
    #[serde(rename = "foodExhaustionLevel")]
    pub food_exhaustion_level:f32,
    #[serde(rename = "foodLevel")]
    pub food_level:i32,
    #[serde(rename = "foodTickTimer")]
    pub food_tick_timer:i32,
    pub ignore_fall_damage_from_current_explosion:bool,
    #[serde(rename = "Inventory")]
    pub inventory:Vec<Item>,
    #[serde(rename = "LastDeathLocation")]
    pub last_death_location:LastDeathLocation,
    #[serde(default)]
    #[serde(rename = "playerGameType")]
    pub player_game_type: i32,
    #[serde(rename = "previousPlayerGameType")]
    pub previous_player_game_type:Option<i32>,

    pub raid_omen_position:Option<Vec<i32>>,
    #[serde(default)]
    #[serde(rename = "recipeBook")]
    pub recipe_book:RecipeBook,
    pub respawn:Respawn,
    // #[serde(rename = "RootVehicle")]
    // pub root_vehicle:Option<EntityENum>,
    #[serde(rename = "Score")]
    pub score:i32,
    #[serde(rename = "seenCredits")]
    pub seen_credits:bool,
    #[serde(rename = "SelectedItem")]
    pub selected_item:Option<Item>,
    #[serde(rename = "SelectedItemSlot")]
    pub selected_item_slot:i32,

    // #[serde(rename="ShoulderEntityLeft")]
    // pub shoulder_entity_left:Option<EntityENum>,
    // #[serde(rename="ShoulderEntityRight")]
    // pub shoulder_entity_right:Option<EntityENum>,

    #[serde(rename= "SleepTimer")]
    pub sleep_timer:i16,

    pub spawn_extra_particles_on_fall:Option<bool>,
    #[serde(default)]
    pub warden_spawn_tracker:WardenSpawnTracker,
    
    #[serde(rename="XpLevel")]
    pub xp_level:i32,
    #[serde(rename="XpP")]
    pub xp_p:i32,
    #[serde(rename="XpSeed")]
    pub xp_seed:f32,
    #[serde(rename="XpTotal")]
    pub xp_total:i32,
}
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct WardenSpawnTracker{
    #[serde(default)]
    pub cooldown_ticks:i32,
    #[serde(default)]
    pub ticks_since_last_warning:i32,
    #[serde(default)]
    pub warning_level:i32
}
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct Respawn{
    pub pos:Vec<i32>,
    #[serde(default)]
    pub pitch:f32,
    #[serde(default)]
    pub yaw:f32,
    pub dimension:String,
    #[serde(default)]
    pub forced:bool,
}
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct RecipeBook{
    #[serde(rename="isBlastingFurnaceFilteringCraftable")]
    pub is_blasting_furnace_filtering_craftable:bool,
    #[serde(rename="isBlastingFurnaceGuiOpen")]
    pub is_blasting_furnace_gui_open:bool,
    #[serde(rename="isFilteringCraftable")]
    pub is_filtering_craftable:bool,
    #[serde(rename="isFurnaceFilteringCraftable")]
    pub is_furnace_filtering_craftable:bool,
    #[serde(rename="isFurnaceGuiOpen")]
    pub is_furnace_gui_open:bool,
    #[serde(rename="isGuiOpen")]
    pub is_gui_open:bool,
    #[serde(rename="isSmokerFilteringCraftable")]
    pub is_smoker_filtering_craftable:bool,
    #[serde(rename="isSmokerGuiOpen")]
    pub is_smoker_gui_open:bool,
    pub recipes:Vec<String>,
    #[serde(rename="toBeDisplayed")]
    pub to_be_displayed:Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastDeathLocation{
    pub dimension:String,
    pub pos:Vec<i32>,
}
impl Default for LastDeathLocation {
    fn default() -> Self {
        Self { dimension: "minecraft:world".to_string(), pos: vec![0,0,0] }
    }
}

// 默认值函数
fn default_data_version() -> i32 {
    -1 // 根据注释，如果不存在则认为是-1
}

impl Default for Player {
    fn default() -> Self {
        Self {
            avatar: Avatar::default(), // 假设 Avatar 实现了 Default
            data_version: default_data_version(),
            abilities: Abilities::default(),
            current_explosion_impact_pos: None,
            current_impulse_context_reset_grace_time: 0,
            dimension: None,
            ender_pearls: None,
            player_game_type: 0,
            ender_items: Default::default(),
            entered_nether_pos: Default::default(),
            food_exhaustion_level: Default::default(),
            food_level: Default::default(),
            food_tick_timer: Default::default(),
            ignore_fall_damage_from_current_explosion: Default::default(),
            inventory: Default::default(),
            last_death_location: Default::default(),
            previous_player_game_type: Default::default(),
            raid_omen_position: Default::default(),
            recipe_book: Default::default(),
            respawn: Default::default(),
            score: Default::default(),
            seen_credits: Default::default(),
            selected_item: Default::default(),
            selected_item_slot: Default::default(),
            sleep_timer: Default::default(),
            spawn_extra_particles_on_fall: Default::default(),
            warden_spawn_tracker: Default::default(),
            xp_level: Default::default(),
            xp_p: Default::default(),
            xp_seed: Default::default(),
            xp_total: Default::default(),
        }
    }
}

impl Player {
    pub fn new(uuid: uuid::Uuid) -> Player {
        // 这里需要根据实际情况创建 Avatar
        // 暂时使用默认值，您可能需要修改这里
        let mut p: Player = Default::default();
        p.avatar.living_entity.entity.uuid = types::NbtUuid(uuid);
        p
    }
}

/// 玩家能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abilities {
    /// 玩家是否正在飞行。
    #[serde(default)]
    
    pub flying: bool,
    /// 玩家的飞行速度。如果此项不存在则游戏认为此项是0.05。
    #[serde(rename = "flySpeed")]
    #[serde(default = "default_fly_speed")]
    pub fly_speed: f32,
    /// 玩家是否可以立刻摧毁方块、选取方块时是否允许保存方块实体数据、使用铁砧是否不消耗经验值且不会过于昂贵、是否立刻破坏载具等。
    #[serde(default)]
    
    pub instabuild: bool,
    /// 玩家是否能抵抗绝大多数伤害。如果为true，玩家不消耗饥饿值、通过下界传送门的时间被改变、身上的火可以快速熄灭，且只会受到带有#bypasses_invulnerability标签的伤害。
    #[serde(default)]
    
    pub invulnerable: bool,
    /// 玩家是否可以摧毁、放置和调整方块和盔甲架。如果此项不存在则游戏认为此项是true。
    #[serde(default = "default_may_build")]
    
    #[serde(rename = "mayBuild")]
    pub may_build: bool,
    /// 玩家是否能飞行，阻止玩家因为飞行而被服务器踢出。
    #[serde(default)]
    
    #[serde(rename = "mayfly")]
    pub may_fly: bool,
    /// 步行速度。如果单精度浮点数flySpeed不存在则游戏认为此项是0.1。
    #[serde(rename = "walkSpeed")]
    #[serde(default = "default_walk_speed")]
    pub walk_speed: f32,
}

// Abilities 的默认值函数
fn default_fly_speed() -> f32 {
    0.05
}

fn default_walk_speed() -> f32 {
    0.1
}

fn default_may_build() -> bool {
    true
}

impl Default for Abilities {
    fn default() -> Self {
        Self {
            flying: false,
            fly_speed: default_fly_speed(),
            instabuild: false,
            invulnerable: false,
            may_build: default_may_build(),
            may_fly: false,
            walk_speed: default_walk_speed(),
        }
    }
}

/// 玩家被任何爆炸击退时，此数据才存在，表示被爆炸击退时的坐标。当玩家落地、陷入方块、切换为创造模式、使用末影珍珠和紫颂果传送时此值被清除。
#[derive(Debug, Clone, Default)]
pub struct CurrentExplosionImpactPos {
    /// 击退时的X坐标。玩家使用重锤猛击时，如果Y坐标高于玩家当前位置，则会被设置为玩家当前的X坐标。
    pub x: f64,
    /// 击退时的Y坐标。玩家使用重锤猛击时，如果此值高于玩家当前位置，则会被设置为玩家当前的Y坐标。
    pub y: f64,
    /// 击退时的Z坐标。玩家使用重锤猛击时，如果Y坐标高于玩家当前位置，则会被设置为玩家当前的Z坐标。
    pub z: f64,
}

// 手动实现 Serialize：将 Pos 转为 List<Double>
impl Serialize for CurrentExplosionImpactPos {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(3))?; // 固定长度3
        seq.serialize_element(&self.x)?;
        seq.serialize_element(&self.y)?;
        seq.serialize_element(&self.z)?;
        seq.end()
    }
}

// 手动实现 Deserialize：从 List<Double> 解析为 Pos
impl<'de> Deserialize<'de> for CurrentExplosionImpactPos {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 反序列化为 Vec<f64>
        let values = <Vec<f64>>::deserialize(deserializer)?;

        // 验证列表长度必须为3（X/Y/Z）
        if values.len() != 3 {
            return Err(serde::de::Error::invalid_length(
                values.len(),
                &"a sequence of exactly 3 elements (x, y, z)",
            ));
        }

        Ok(CurrentExplosionImpactPos {
            x: values[0],
            y: values[1],
            z: values[2],
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnderPearls {
    #[serde(default)]
    pub ender_pearl_dimension: Option<String>,
    // #[serde(flatten)]
    // pub thrown_enderpearl: ThrownEnderpearl,
}
