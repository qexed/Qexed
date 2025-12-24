use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 规则配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleConfig {
    pub version: i32,
    pub player_save: PlayerSave,
    /// 默认全局世界规则
    pub global_world: WorldRule,
    /// 非默认情况下的全局世界规则
    pub world: Vec<WorldRule>,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            version: 0,
            player_save: Default::default(),
            global_world: Default::default(),
            world: vec![],
        }
    }
}

impl RuleConfig {
    /// 保存配置到文件
    pub async fn save_to_file(&self) -> Result<(), std::io::Error> {
        use std::fs;
        use std::path::Path;
        
        let path = Path::new("./config/qexed_rule/");
        if !path.exists() {
            fs::create_dir_all(path)?;
        }
        
        let file_path = path.join("config.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(file_path, json)?;
        
        Ok(())
    }
    
    /// 从文件加载配置
    pub async fn load_from_file() -> Result<Self, std::io::Error> {
        use std::fs;
        use std::path::Path;
        
        let file_path = Path::new("./config/qexed_rule/config.json");
        if !file_path.exists() {
            return Ok(Self::default());
        }
        
        let json = fs::read_to_string(file_path)?;
        let config = serde_json::from_str(&json)?;
        Ok(config)
    }
}

/// 玩家保存配置
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PlayerSave {
    /// 是否启用玩家数限制
    pub max_player_limit: bool,
    /// 最大玩家数量限制
    pub max_players: u32,
}

/// Minecraft 原版游戏规则
/// 参考: https://zh.minecraft.wiki/w/游戏规则
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldRule {
    // ==================== 世界更新类规则 ====================
    /// 游戏内时间流逝 | 是否进行昼夜更替和月相变化
    #[serde(default = "default_true")]
    pub advance_time: bool,
    
    /// 天气更替 | 天气是否变化
    #[serde(default = "default_true")]
    pub advance_weather: bool,
    
    /// 火焰蔓延半径 | 玩家周围火焰可以蔓延的方块半径
    #[serde(default = "default_i32_128")]
    pub fire_spread_radius_around_player: i32,
    
    /// 允许流动熔岩转化为熔岩源 | 流动熔岩在两面与熔岩源相邻时转化为熔岩源
    #[serde(default = "default_false")]
    pub lava_source_conversion: bool,
    
    /// 随机刻速率 | 每游戏刻每区段中随机的方块刻发生的频率
    #[serde(default = "default_i32_3")]
    pub random_tick_speed: i32,
    
    /// 积雪厚度 | 下雪时可在一格方块空间内堆积的雪的最高层数
    #[serde(default = "default_i32_1")]
    pub max_snow_accumulation_height: i32,
    
    /// 藤蔓蔓延 | 控制藤蔓方块能否随机向相邻的方块蔓延
    #[serde(default = "default_true")]
    pub spread_vines: bool,
    
    /// 允许流动水转化为水源 | 流动水在两面与水源相邻时转化为水源
    #[serde(default = "default_true")]
    pub water_source_conversion: bool,
    
    // ==================== 玩家类规则 ====================
    /// 溺水伤害 | 玩家是否承受窒息伤害
    #[serde(default = "default_true")]
    pub drowning_damage: bool,
    
    /// 启用鞘翅移动检测 | 是否让服务器停止检查使用鞘翅玩家的移动速度
    #[serde(default = "default_true")]
    pub elytra_movement_check: bool,
    
    /// 掷出的末影珍珠在死亡时消失 | 玩家投掷的末影珍珠是否在玩家死亡时消失
    #[serde(default = "default_true")]
    pub ender_pearls_vanish_on_death: bool,
    
    /// 摔落伤害 | 玩家是否承受摔落伤害
    #[serde(default = "default_true")]
    pub fall_damage: bool,
    
    /// 火焰伤害 | 玩家是否承受火焰伤害
    #[serde(default = "default_true")]
    pub fire_damage: bool,
    
    /// 冰冻伤害 | 玩家是否承受冰冻伤害
    #[serde(default = "default_true")]
    pub freeze_damage: bool,
    
    /// 立即重生 | 玩家死亡时是否不显示死亡界面直接重生
    #[serde(default = "default_false")]
    pub immediate_respawn: bool,
    
    /// 死亡后保留物品栏 | 玩家死亡后是否保留物品栏物品、经验
    #[serde(default = "default_false")]
    pub keep_inventory: bool,
    
    /// 合成需要配方 | 若启用，玩家只能使用已解锁的配方合成
    #[serde(default = "default_false")]
    pub limited_crafting: bool,
    
    /// 启用玩家定位栏 | 启用后，屏幕上会显示指示玩家方位的定位栏
    #[serde(default = "default_true")]
    pub locator_bar: bool,
    
    /// 生命值自然恢复 | 玩家是否能在饥饿值足够时自然恢复生命值
    #[serde(default = "default_true")]
    pub natural_health_regeneration: bool,
    
    /// 启用玩家移动检测 | 是否让服务器停止检查并限制玩家的移动速度
    #[serde(default = "default_true")]
    pub player_movement_check: bool,
    
    /// 创造模式下玩家在下界传送门中等待的时间 | 创造模式下的玩家需要待在下界传送门内多少游戏刻才能进入另一个维度
    #[serde(default = "default_i32_0")]
    pub players_nether_portal_creative_delay: i32,
    
    /// 非创造模式下玩家在下界传送门中等待的时间 | 非创造模式下的玩家需要待在下界传送门内多少游戏刻才能进入另一个维度
    #[serde(default = "default_i32_80")]
    pub players_nether_portal_default_delay: i32,
    
    /// 入睡占比 | 跳过夜晚所需的入睡玩家占比
    #[serde(default = "default_i32_100")]
    pub players_sleeping_percentage: i32,
    
    /// 启用PvP | 控制玩家间能否互相伤害
    #[serde(default = "default_true")]
    pub pvp: bool,
    
    /// 重生点半径 | 首次进入服务器的玩家和没有重生点的死亡玩家在重生时与世界出生点坐标的距离
    #[serde(default = "default_i32_10")]
    pub respawn_radius: i32,
    
    /// 允许旁观者生成地形 | 是否允许旁观模式的玩家生成区块
    #[serde(default = "default_true")]
    pub spectators_generate_chunks: bool,
    
    // ==================== 生物类规则 ====================
    /// 宽恕死亡玩家 | 愤怒的中立生物将在其目标玩家于附近死亡后息怒
    #[serde(default = "default_true")]
    pub forgive_dead_players: bool,
    
    /// 实体挤压上限 | 控制挤压机制。同一位置的可推动实体的上限超过该游戏规则的数量时会引发挤压伤害
    #[serde(default = "default_i32_24")]
    pub max_entity_cramming: i32,
    
    /// 允许破坏性生物行为 | 生物是否能够进行破坏性行为
    #[serde(default = "default_true")]
    pub mob_griefing: bool,
    
    /// 无差别愤怒 | 被激怒的条件敌对生物是否攻击附近任何玩家
    #[serde(default = "default_false")]
    pub universal_anger: bool,
    
    // ==================== 掉落类规则 ====================
    /// 方块掉落 | 控制破坏方块后是否掉落资源，包括经验球
    #[serde(default = "default_true")]
    pub block_drops: bool,
    
    /// 在方块交互爆炸中，一些方块不会掉落战利品 | 由床或重生锚爆炸炸毁的方块是否会有概率不掉落
    #[serde(default = "default_true")]
    pub block_explosion_drop_decay: bool,
    
    /// 非生物实体掉落 | 控制矿车、物品展示框、船等的物品掉落
    #[serde(default = "default_true")]
    pub entity_drops: bool,
    
    /// 生物战利品掉落 | 控制生物死亡后是否掉落资源，包括经验球
    #[serde(default = "default_true")]
    pub mob_drops: bool,
    
    /// 在生物爆炸中，一些方块不会掉落战利品 | 由生物源爆炸炸毁的方块是否会有概率不掉落
    #[serde(default = "default_true")]
    pub mob_explosion_drop_decay: bool,
    
    /// 弹射物能否破坏方块 | 控制弹射物能否破坏可被其破坏的方块
    #[serde(default = "default_true")]
    pub projectiles_can_break_blocks: bool,
    
    /// 在TNT爆炸中，一些方块不会掉落战利品 | 由TNT爆炸炸毁的方块是否会有概率不掉落
    #[serde(default = "default_false")]
    pub tnt_explosion_drop_decay: bool,
    
    // ==================== 聊天类规则 ====================
    /// 广播命令方块输出 | 命令方块执行命令时是否在聊天框中向管理员显示
    #[serde(default = "default_true")]
    pub command_block_output: bool,
    
    /// 通告管理员命令 | 是否在服务器日志中记录管理员使用过的命令
    #[serde(default = "default_true")]
    pub log_admin_commands: bool,
    
    /// 发送命令反馈 | 玩家执行命令的返回信息是否在聊天框中显示
    #[serde(default = "default_true")]
    pub send_command_feedback: bool,
    
    /// 进度通知 | 是否在聊天框中公告玩家进度的达成
    #[serde(default = "default_true")]
    pub show_advancement_messages: bool,
    
    /// 显示死亡消息 | 是否在聊天框中显示玩家的死亡消息
    #[serde(default = "default_true")]
    pub show_death_messages: bool,
    
    // ==================== 杂项类规则 ====================
    /// 允许进入下界 | 控制玩家能否进入下界
    #[serde(default = "default_true")]
    pub allow_entering_nether_using_portals: bool,
    
    /// 启用命令方块 | 命令方块在游戏中是否被启用
    #[serde(default = "default_true")]
    pub command_blocks_work: bool,
    
    /// 全局声音事件 | 特定游戏事件发生时，声音可在所有地方听见
    #[serde(default = "default_true")]
    pub global_sound_events: bool,
    
    /// 命令修改方块数量限制 | 单条命令最多能更改的方块数量
    #[serde(default = "default_i32_32768")]
    pub max_block_modifications: i32,
    
    /// 命令上下文数量限制 | 决定了命令能使用的命令上下文的总数量
    #[serde(default = "default_i32_65536")]
    pub max_command_forks: i32,
    
    /// 命令连锁执行数量限制 | 应用于命令方块链和函数
    #[serde(default = "default_i32_65536")]
    pub max_command_sequence_length: i32,
    
    /// 矿车最大速度 | 矿车在地面上移动的默认最大速度
    #[serde(default = "default_i32_8")]
    pub max_minecart_speed: i32,
    
    /// 启用袭击 | 是否禁用袭击
    #[serde(default = "default_true")]
    pub raids: bool,
    
    /// 简化调试信息 | 调试屏幕是否简化而非显示详细信息
    #[serde(default = "default_false")]
    pub reduced_debug_info: bool,
    
    /// 启用刷怪笼方块 | 是否允许刷怪笼与试炼刷怪笼运作
    #[serde(default = "default_true")]
    pub spawner_blocks_work: bool,
    
    /// 允许TNT被点燃并爆炸 | TNT是否会爆炸
    #[serde(default = "default_true")]
    pub tnt_explodes: bool,
    
    // ==================== 生成类规则 ====================
    /// 生成生物 | 生物是否自然生成
    #[serde(default = "default_true")]
    pub spawn_mobs: bool,
    
    /// 生成怪物 | 控制怪物能否自然生成
    #[serde(default = "default_true")]
    pub spawn_monsters: bool,
    
    /// 生成灾厄巡逻队 | 控制灾厄巡逻队的生成
    #[serde(default = "default_true")]
    pub spawn_patrols: bool,
    
    /// 生成幻翼 | 幻翼是否在夜晚生成
    #[serde(default = "default_true")]
    pub spawn_phantoms: bool,
    
    /// 生成流浪商人 | 控制流浪商人的生成
    #[serde(default = "default_true")]
    pub spawn_wandering_traders: bool,
    
    /// 生成监守者 | 监守者是否生成
    #[serde(default = "default_true")]
    pub spawn_wardens: bool,

    pub world_id: Option<Uuid>,
    pub inherits_from: Option<String>,
    pub is_default: bool,
}

impl Default for WorldRule {
    fn default() -> Self {
        Self {
            // 世界更新
            advance_time: true,
            advance_weather: true,
            fire_spread_radius_around_player: 128,
            lava_source_conversion: false,
            random_tick_speed: 3,
            max_snow_accumulation_height: 1,
            spread_vines: true,
            water_source_conversion: true,
            
            // 玩家
            drowning_damage: true,
            elytra_movement_check: true,
            ender_pearls_vanish_on_death: true,
            fall_damage: true,
            fire_damage: true,
            freeze_damage: true,
            immediate_respawn: false,
            keep_inventory: false,
            limited_crafting: false,
            locator_bar: true,
            natural_health_regeneration: true,
            player_movement_check: true,
            players_nether_portal_creative_delay: 0,
            players_nether_portal_default_delay: 80,
            players_sleeping_percentage: 100,
            pvp: true,
            respawn_radius: 10,
            spectators_generate_chunks: true,
            
            // 生物
            forgive_dead_players: true,
            max_entity_cramming: 24,
            mob_griefing: true,
            universal_anger: false,
            
            // 掉落
            block_drops: true,
            block_explosion_drop_decay: true,
            entity_drops: true,
            mob_drops: true,
            mob_explosion_drop_decay: true,
            projectiles_can_break_blocks: true,
            tnt_explosion_drop_decay: false,
            
            // 聊天
            command_block_output: true,
            log_admin_commands: true,
            send_command_feedback: true,
            show_advancement_messages: true,
            show_death_messages: true,
            
            // 杂项
            allow_entering_nether_using_portals: true,
            command_blocks_work: true,
            global_sound_events: true,
            max_block_modifications: 32768,
            max_command_forks: 65536,
            max_command_sequence_length: 65536,
            max_minecart_speed: 8,
            raids: true,
            reduced_debug_info: false,
            spawner_blocks_work: true,
            tnt_explodes: true,
            
            // 生成
            spawn_mobs: true,
            spawn_monsters: true,
            spawn_patrols: true,
            spawn_phantoms: true,
            spawn_wandering_traders: true,
            spawn_wardens: true,
            world_id: None,
            inherits_from: None,
            is_default: false,
        }
    }
}

// 辅助函数，用于指定默认值
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_i32_0() -> i32 { 0 }
fn default_i32_1() -> i32 { 1 }
fn default_i32_3() -> i32 { 3 }
fn default_i32_8() -> i32 { 8 }
fn default_i32_10() -> i32 { 10 }
fn default_i32_24() -> i32 { 24 }
fn default_i32_80() -> i32 { 80 }
fn default_i32_100() -> i32 { 100 }
fn default_i32_128() -> i32 { 128 }
fn default_i32_32768() -> i32 { 32768 }
fn default_i32_65536() -> i32 { 65536 }