//! v4 全量游戏事件的 v6 强类型版本。
//!
//! v4 时代事件经 WASM C-ABI 导出符号 + serde 载荷传递；v6 中同一语义改为
//! `dyn Event` 对象 + type_name 分发。事件名与载荷字段保持 v4 语义：
//! 移植 v4 插件时逻辑照抄，把 `qexed_plugin_<event>(ptr,len)` 函数改写成
//! `EventHandler<对应事件>` 实现即可。
//!
//! 事件四类：玩家会话（join/leave/chat/command）、玩家行为（move/tick/
//! input/attack/interact/use_item/death/pickup…）、服务器生命周期（chunk/
//! config/language）、查询-响应型（enchanting/crafting/mining_speed/
//! placeholders…，响应字段为 `&mut` 或 cancel 即接管，见各事件注释）。

use crate::payload::*;
use crate::PlayerInfo;

// ---------------------------------------------------------------------------
// 玩家会话
// ---------------------------------------------------------------------------

/// 插件全部加载完成（服务器启动路径触发一次）。取消无意义。
#[derive(Debug, Clone, Copy, Default)]
pub struct PluginLoadFinishEvent;

impl crate::Event for PluginLoadFinishEvent {}

/// 玩家进入 PLAY 阶段（世界流送完成前即触发，等同 v4 player_join）。
#[derive(Clone, Debug)]
pub struct PlayerJoinEvent {
    pub player: PlayerInfo,
}

impl PlayerJoinEvent {
    pub fn new(player: PlayerInfo) -> Self {
        Self { player }
    }
}

/// 玩家离开（连接关闭，含超时/错误断开）。
#[derive(Clone, Debug)]
pub struct PlayerLeaveEvent {
    pub player: PlayerInfo,
}

impl PlayerLeaveEvent {
    pub fn new(player: PlayerInfo) -> Self {
        Self { player }
    }
}

/// 玩家聊天（cancel = 该消息不进入后续聊天流程）。
#[derive(Clone, Debug)]
pub struct PlayerChatEvent {
    pub player: PlayerInfo,
    pub message: String,
}

impl PlayerChatEvent {
    pub fn new(player: PlayerInfo, message: impl Into<String>) -> Self {
        Self { player, message: message.into() }
    }
}

/// 玩家执行了命令。`command` 不含前导 `/`；参数为原始文本。
/// cancel = 命令不再交给内置指令树处理。
#[derive(Clone, Debug)]
pub struct PlayerCommandEvent {
    pub player: PlayerInfo,
    pub command: String,
}

impl PlayerCommandEvent {
    pub fn new(player: PlayerInfo, command: impl Into<String>) -> Self {
        Self { player, command: command.into() }
    }
}

/// 客户端主动请求执行命令（同 v4 commands 事件语义）。
#[derive(Clone, Debug)]
pub struct CommandsEvent {
    pub player: PlayerInfo,
    /// 不含前导 `/` 的完整命令行。
    pub input: String,
}

/// 插件注册命令被客户端执行（不含前导 `/`）。cancel = 命令不交给指令树。
#[derive(Clone, Debug)]
pub struct ChatCommandEvent {
    pub player: PlayerInfo,
    pub command: String,
    pub argument: String,
}

impl ChatCommandEvent {
    pub fn new(player: PlayerInfo, command: impl Into<String>, argument: impl Into<String>) -> Self {
        Self { player, command: command.into(), argument: argument.into() }
    }
}

// ---------------------------------------------------------------------------
// 玩家行为
// ---------------------------------------------------------------------------

/// 玩家移动（位置或视角变化）。cancel = 位置更新不落地。
#[derive(Clone, Debug)]
pub struct PlayerMoveEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub previous: PlayerPositionPayload,
    pub position: PlayerPositionPayload,
}

/// 玩家 tick：v4 由宿主按固定节奏触发；v6 首批接线以移动/输入包近似，
/// `tick_millis` 为自上一事件以来的毫秒数。
#[derive(Clone, Debug)]
pub struct PlayerTickEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub tick_millis: u64,
}

/// 玩家输入状态变化（PlayerInput 包）。
#[derive(Clone, Debug)]
pub struct PlayerInputEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub previous_input: PlayerInputState,
    pub input: PlayerInputState,
}

/// 玩家攻击实体。`damage` 为宿主基准伤害；事件响应可改写或 cancel。
#[derive(Clone, Debug)]
pub struct PlayerAttackEvent {
    pub player: PlayerInfo,
    pub target_entity_id: i32,
    pub damage: f32,
    /// v6 首批无运行时实体，恒为 0；保留字段对齐 v4 载荷。
    pub critical: bool,
}

/// 玩家走到新方块上（含跨块边界判定由宿主完成）。
#[derive(Clone, Debug)]
pub struct PlayerBlockStepEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub block: BlockStepPosition,
}

/// 玩家对方块伸手（UseItemOn）。响应 `interact` 默认 false；置 true 表示
/// 由插件接管（宿主不再派发内置行为）。
#[derive(Clone, Debug)]
pub struct PlayerBlockInteractEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub block: BlockDropPosition,
    /// v6 首批无方块读取，占位 0；保留对齐 v4 载荷。
    pub block_state: i32,
    pub block_name: String,
    pub hand: String,
    pub sequence: i32,
    pub face: String,
    pub face_id: i32,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_z: f32,
    pub inside_block: bool,
    pub world_border_hit: bool,
    pub input: PlayerInputState,
    /// 置 true = 插件接管本次交互。
    pub interact: bool,
}

/// 玩家使用物品（手持右键，未指向方块）。
#[derive(Clone, Debug)]
pub struct PlayerUseItemEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub hand: String,
    /// v4 语义：use / release …；v6 首批恒为 "use"。
    pub action: String,
    pub item: ItemStackPayload,
    pub sequence: i32,
    pub yaw: f32,
    pub pitch: f32,
}

/// 抛射物命中玩家（SpawnProjectile 的宿主模拟回调）。
#[derive(Clone, Debug)]
pub struct ProjectileHitPlayerEvent {
    pub shooter: PlayerInfo,
    pub target: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub projectile_entity_id: i32,
    pub projectile_kind: String,
    pub configured_event: String,
    pub tag: String,
}

/// 玩家死亡。响应 `cancel` = 免死（v4 skip_death_respawn 语义），`message`
/// 非空 = 覆盖死亡消息。
#[derive(Clone, Debug)]
pub struct PlayerDeathEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub cause: String,
    pub source_entity_id: i32,
    /// 插件置 true 免死。
    pub cancel: bool,
    /// 插件填写的死亡/复活提示。
    pub message: String,
}

/// 玩家拾取物品。响应 `cancel` = 不拾取，`consume` = 消耗掉实体。
#[derive(Clone, Debug)]
pub struct PlayerItemPickupEvent {
    pub player: PlayerInfo,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub item_entity_id: i32,
    pub item_id: Option<i32>,
    pub item_name: String,
    pub count: i32,
    pub cancel: bool,
    pub consume: bool,
}

/// 玩家挖掘速度查询（返回 speed/multiplier/add 任一即接管）。
#[derive(Clone, Debug)]
pub struct MiningSpeedEvent {
    pub player: PlayerInfo,
    pub query: MiningSpeedQuery,
    /// 插件响应字段。
    pub response: MiningSpeedResponse,
}

/// 方块掉落查询（replace=true + items 即接管掉落表）。
#[derive(Clone, Debug)]
pub struct BlockDropsEvent {
    pub query: BlockDropQuery,
    pub response: BlockDropResponse,
}

// ---------------------------------------------------------------------------
// NPC / 实体
// ---------------------------------------------------------------------------

/// NPC 被玩家交互。`configured_event` 是 upsert 时注册的事件名。
#[derive(Clone, Debug)]
pub struct NpcInteractEvent {
    pub payload: NpcInteractPayload,
}

/// NPC 突变请求：插件返回要执行的操作序列。
#[derive(Debug)]
pub struct NpcMutationsEvent {
    pub query: NpcMutationQuery,
    /// Mutex：插件在 handler 内回写操作（fire 侧随后统一收集）。
    pub response: std::sync::Mutex<NpcMutationResponse>,
}

/// 自定义实体注册查询（宿主启动/插件启用后收集）。
#[derive(Clone, Debug)]
pub struct CustomEntitiesEvent {
    pub definitions: Vec<CustomEntityDefinition>,
}

/// 实体 AI tick：宿主按节奏询问插件该实体这一拍做什么。
#[derive(Debug)]
pub struct EntityAiTickEvent {
    pub query: EntityAiTickQuery,
    /// Mutex：插件在 handler 内回写操作（fire 侧随后统一收集）。
    pub response: std::sync::Mutex<EntityAiTickResponse>,
}

// ---------------------------------------------------------------------------
// 查询-响应
// ---------------------------------------------------------------------------

/// 占位符解析：插件认领 `%key%` 并给出替换值。
#[derive(Debug)]
pub struct PlaceholdersEvent {
    pub query: PlaceholderQuery,
    /// Mutex：插件在 handler 内回写替换值（fire 侧随后统一收集）。
    pub response: std::sync::Mutex<PlaceholderResponse>,
}

/// 附魔选项查询。
#[derive(Clone, Debug)]
pub struct EnchantingOptionsEvent {
    pub query: EnchantingQuery,
    pub response: EnchantingResponse,
}

/// 合成表查询。
#[derive(Clone, Debug)]
pub struct CraftingRecipeEvent {
    pub query: CraftingRecipeQuery,
    pub response: CraftingRecipeResponse,
}

/// 单件合成查询。
#[derive(Clone, Debug)]
pub struct CraftItemEvent {
    pub query: CraftItemQuery,
    pub response: CraftItemResponse,
}

/// 熔炉配方查询。
#[derive(Clone, Debug)]
pub struct FurnaceRecipeEvent {
    pub query: FurnaceRecipeQuery,
    pub response: FurnaceRecipeResponse,
}

/// 熔炉燃烧/冶炼 tick。
#[derive(Clone, Debug)]
pub struct FurnaceTickEvent {
    pub payload: FurnaceTickPayload,
}

/// 工具耐久查询。
#[derive(Clone, Debug)]
pub struct ItemDurabilityEvent {
    pub query: ItemDurabilityQuery,
    pub response: ItemDurabilityResponse,
}

/// 音效播放请求（插件可改写或 cancel）。
#[derive(Clone, Debug)]
pub struct SoundEvent {
    pub payload: SoundPayload,
    pub response: SoundResponse,
}

/// 基岩表单回包。
#[derive(Clone, Debug)]
pub struct BedrockFormResponseEvent {
    pub payload: BedrockFormResponsePayload,
}

/// 代理切换结果。
#[derive(Clone, Debug)]
pub struct ProxyConnectResultEvent {
    pub payload: ProxyConnectResultPayload,
}

/// 跨插件 API 调用（v4 api_call 事件：provider 侧接收）。
#[derive(Clone, Debug)]
pub struct ApiCallEvent {
    pub query: PluginApiCallQuery,
    pub response: PluginApiCallResponse,
}

// ---------------------------------------------------------------------------
// 服务器生命周期
// ---------------------------------------------------------------------------

/// 区块加载。
#[derive(Clone, Debug)]
pub struct ChunkLoadEvent {
    pub chunk: ChunkPayload,
}

/// 区块卸载。
#[derive(Clone, Debug)]
pub struct ChunkUnloadEvent {
    pub chunk: ChunkPayload,
}

/// 配置文件重载。
#[derive(Clone, Debug)]
pub struct ConfigReloadEvent {
    pub payload: ConfigReloadPayload,
}

/// 玩家语言切换。
#[derive(Clone, Debug)]
pub struct LanguageChangeEvent {
    pub payload: LanguagePayload,
}

// -------------------------------------------------------------------------
// Event 标记实现（type_name 分发要求全部事件实现 Event）
// -------------------------------------------------------------------------

impl crate::Event for PlayerJoinEvent {}
impl crate::Event for PlayerLeaveEvent {}
impl crate::Event for PlayerChatEvent {}
impl crate::Event for PlayerCommandEvent {}
impl crate::Event for CommandsEvent {}
impl crate::Event for ChatCommandEvent {}
impl crate::Event for PlayerMoveEvent {}
impl crate::Event for PlayerTickEvent {}
impl crate::Event for PlayerInputEvent {}
impl crate::Event for PlayerAttackEvent {}
impl crate::Event for PlayerBlockStepEvent {}
impl crate::Event for PlayerBlockInteractEvent {}
impl crate::Event for PlayerUseItemEvent {}
impl crate::Event for ProjectileHitPlayerEvent {}
impl crate::Event for PlayerDeathEvent {}
impl crate::Event for PlayerItemPickupEvent {}
impl crate::Event for MiningSpeedEvent {}
impl crate::Event for BlockDropsEvent {}
impl crate::Event for NpcInteractEvent {}
impl crate::Event for NpcMutationsEvent {}
impl crate::Event for CustomEntitiesEvent {}
impl crate::Event for EntityAiTickEvent {}
impl crate::Event for PlaceholdersEvent {}
impl crate::Event for EnchantingOptionsEvent {}
impl crate::Event for CraftingRecipeEvent {}
impl crate::Event for CraftItemEvent {}
impl crate::Event for FurnaceRecipeEvent {}
impl crate::Event for FurnaceTickEvent {}
impl crate::Event for ItemDurabilityEvent {}
impl crate::Event for SoundEvent {}
impl crate::Event for BedrockFormResponseEvent {}
impl crate::Event for ProxyConnectResultEvent {}
impl crate::Event for ApiCallEvent {}
impl crate::Event for ChunkLoadEvent {}
impl crate::Event for ChunkUnloadEvent {}
impl crate::Event for ConfigReloadEvent {}
impl crate::Event for LanguageChangeEvent {}
