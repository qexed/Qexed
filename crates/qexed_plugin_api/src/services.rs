//! 宿主服务 trait：v4 宿主调用（host.rs WASM import）的 v6 原生类型版。
//!
//! v4 里插件通过 C-ABI import + JSON/制表符编码字符串调用宿主；v6 中同一
//! 能力改为 [`crate::HostApi`] 组合出来的强类型 trait。方法名与 v4 SDK
//! `pub fn` 一一对应，移植时把 `qexed_plugin_sdk::world_set_block(...)`
//! 改成 `api.world_set_block(...)`，参数含义不变。

use crate::payload::*;
use crate::PlayerInfo;

/// v4 `WorldEditRegion`：插件向宿主登记一方块编辑区（权限/运行时归属）。
/// 坐标为方块坐标（含端点）；`runtime_only` 区块不落盘。
pub struct WorldEditRegionSpec {
    pub id: String,
    pub dimension: String,
    pub min: (i32, i32, i32),
    pub max: (i32, i32, i32),
    pub allow_player_break: bool,
    pub allow_player_place: bool,
    pub allow_plugin_write: bool,
    pub runtime_only: bool,
}

/// v4 `RuntimeEntity`：插件运行时实体（NPC/展示实体）定义。
/// `key` 由插件命名空间内唯一（如 `"gobang:guide"`）。
#[derive(Clone, Debug)]
pub struct RuntimeEntitySpec {
    pub key: String,
    pub dimension: String,
    pub entity_type: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub display_name: String,
    /// AI 名称（内置或由 CustomEntitiesEvent 注册）；空 = 无 AI。
    pub ai: String,
    /// AI 参数 JSON 对象序列化串。
    pub ai_params_json: String,
    pub auto_jump: bool,
}

// ---------------------------------------------------------------------------
// 世界编辑
// ---------------------------------------------------------------------------

/// 世界编辑桥。v6 实现位于 ActionHub（不触碰 qexed_world）：
/// 方块变更以 BlockUpdate/SectionBlocksUpdate 包推给客户端，并按
/// 编辑区登记缓存，供重登/跨视图重放。
pub trait WorldEditApi: Send + Sync {
    /// 单方块置。返回是否成功。
    fn world_set_block(&self, dimension: &str, pos: (i32, i32, i32), block: &str) -> bool;
    /// 批量置：`dimension, (x,y,z), block` 三元组序列（v4 SDK 同形）。返回成功写入数。
    fn world_set_blocks(&self, blocks: &[(&str, (i32, i32, i32), &str)]) -> usize;
    /// 破坏方块（等于置 air）。返回是否成功。
    fn world_break_block(&self, dimension: &str, pos: (i32, i32, i32)) -> bool;
    /// 登记编辑区。重复 id 覆盖。返回是否成功。
    fn world_register_edit_region(&self, region: &WorldEditRegionSpec) -> bool;
}

// ---------------------------------------------------------------------------
// 运行时实体 / NPC
// ---------------------------------------------------------------------------

/// 运行时实体控制。v6 实现向在线玩家广播 AddEntity/Move/Remove 包。
pub trait EntityControlApi: Send + Sync {
    /// 创建或更新运行时实体。返回是否成功。
    fn entity_upsert(&self, entity: &RuntimeEntitySpec) -> bool;
    /// 带交互事件注册的 upsert（v4 entity_upsert_with_events）。
    fn entity_upsert_with_events(
        &self,
        entity: &RuntimeEntitySpec,
        main_hand_event: &str,
        off_hand_event: &str,
        attack_event: &str,
    ) -> bool;
    /// 平滑移动实体（客户端插值）。返回是否成功。
    fn entity_move(&self, key: &str, dimension: &str, position: NpcPosition) -> bool;
    /// NPC 巡逻单步推进。返回是否成功。
    fn npc_patrol_step(
        &self,
        key: &str,
        dimension: &str,
        current: NpcPosition,
        target: NpcPosition,
        speed: f64,
    ) -> bool;
    /// 移除实体。返回是否成功。
    fn entity_remove(&self, key: &str) -> bool;
}

// ---------------------------------------------------------------------------
// 结构化存储
// ---------------------------------------------------------------------------

/// 插件本地结构化存储（按 plugin_id 隔离，二进制安全）。
/// v6 实现：`storage/<plugin_id>/` 目录下的文件；同步实现（v4 的 async
/// submit/poll 模式在原生 in-process 调用下没有意义，同步即等价）。
pub trait StructuredStorageApi: Send + Sync {
    fn structured_storage_exists(&self, plugin_id: &str, key: &str) -> bool;
    fn structured_storage_get(&self, plugin_id: &str, key: &str) -> Option<Vec<u8>>;
    /// 写入（目录不存在自动建）。返回是否成功。
    fn structured_storage_set(&self, plugin_id: &str, key: &str, value: &[u8]) -> bool;
    /// 删除。返回是否存在过。
    fn structured_storage_delete(&self, plugin_id: &str, key: &str) -> bool;
}

// ---------------------------------------------------------------------------
// 经济
// ---------------------------------------------------------------------------

/// 货币与账本。余额 i64（最小单位），`fractional_digits` 只是展示约定。
/// v6 实现落在 ActionHub 的经济分册（`storage/<plugin>/economy.json`）。
pub trait EconomyApi: Send + Sync {
    /// 注册货币。重复注册返回 false。
    fn economy_register_currency(
        &self,
        plugin_id: &str,
        id: &str,
        name: &str,
        symbol: &str,
        fractional_digits: i32,
    ) -> bool;
    fn economy_currency_info(&self, currency: &str) -> Option<CurrencyInfo>;
    /// 导出某货币全部账本（JSON 字符串，v4 economy_storage 语义）。
    fn economy_storage(&self, currency: &str) -> Option<String>;
    fn economy_balance(&self, player: &str, currency: &str) -> Option<i64>;
    /// 设置余额（可为负）。返回新余额。
    fn economy_set_balance(&self, player: &str, currency: &str, amount: i64) -> Option<i64>;
    /// 存款。返回新余额；货币/玩家不存在为 None。
    fn economy_deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64>;
    /// 取款（余额不足返回 None）。返回新余额。
    fn economy_withdraw(&self, player: &str, currency: &str, amount: i64) -> Option<i64>;
}

/// v4 `CurrencyInfo`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrencyInfo {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub fractional_digits: i32,
}

// ---------------------------------------------------------------------------
// 随机池 / 抽奖
// ---------------------------------------------------------------------------

/// 加权随机抽取。`kind` 为池类别（block/item/chest/…，宿主只做透传）；
/// `pool_id` 相同且 `precompute_count` 更大时重建缓存（v4 语义）。
/// 返回抽中的 value。
pub trait RandomPoolApi: Send + Sync {
    fn random_pool_roll(
        &self,
        kind: &str,
        pool_id: &str,
        entries: &[(&str, u64)],
        precompute_count: usize,
    ) -> Option<String>;
}

// ---------------------------------------------------------------------------
// 占位符
// ---------------------------------------------------------------------------

/// 占位符注册与展开。`%key%` 风格；插件在 on_enable 注册，宿主在文本
/// 下发路径统一展开。
pub trait PlaceholderApi: Send + Sync {
    /// 注册替换器。重复 key 覆盖。返回是否成功。
    fn placeholder_register(
        &self,
        plugin_id: &str,
        key: &str,
        resolver: std::sync::Arc<dyn PlaceholderResolver>,
    ) -> bool;
    /// 注销该插件注册的全部占位符（卸载时宿主自动调用）。
    fn placeholder_unregister_plugin(&self, plugin_id: &str);
    /// 展开文本中的全部已注册占位符。
    fn placeholders_apply(&self, text: &str, player: Option<&PlayerInfo>) -> String;
}

/// 占位符替换器：`key` 匹配 `%<key>%`。
pub trait PlaceholderResolver: Send + Sync {
    fn resolve(&self, player: Option<&PlayerInfo>, context: &PlaceholderContext) -> Option<String>;
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

/// HTTP 客户端（v4 http_request 语义：阻塞式单请求）。
pub trait HttpClientApi: Send + Sync {
    fn http_request(&self, request: &HttpRequest) -> Option<HttpResponse>;
}

// ---------------------------------------------------------------------------
// 路径查找
// ---------------------------------------------------------------------------

/// 路径查找（v4 pathfinding_find：query JSON → 响应 JSON）。
pub trait PathfindingApi: Send + Sync {
    fn pathfinding_find(&self, query: &PathfindingQuery) -> Option<PathfindingResponse>;
}

// ---------------------------------------------------------------------------
// Geyser / 基岩
// ---------------------------------------------------------------------------

/// Geyser 玩家信息查询。v6 首批无 Geyser 集成，返回 NotFound。
pub trait GeyserApi: Send + Sync {
    fn geyser_player_info(&self, query: &GeyserPlayerInfoQuery) -> GeyserPlayerInfoResponse;
}

// ---------------------------------------------------------------------------
// 跨插件服务调用
// ---------------------------------------------------------------------------

/// 跨插件 API：provider 注册方法表，consumer 按服务名调用。
/// 字节载荷语义由 provider 自定（v4 习惯为 postcard/JSON，v6 建议 JSON）。
pub trait InterPluginApi: Send + Sync {
    /// 某服务是否有人提供（宿主内置或插件）。
    fn plugin_service_exists(&self, service: &str) -> bool;
    /// 调用服务方法。调用错误（未知服务/方法）返回 None。
    fn plugin_call(&self, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>>;
    /// provider 侧：注册服务方法表。重复注册覆盖。
    fn plugin_register_service(
        &self,
        plugin_id: &str,
        service: &str,
        provider: std::sync::Arc<dyn PluginServiceProvider>,
    ) -> bool;
    /// provider 侧：注销（卸载时宿主自动调用）。
    fn plugin_unregister_plugin(&self, plugin_id: &str);
}

/// 服务提供者。`method` 空串 = 默认入口（对齐 v4 call 单方法习惯）。
pub trait PluginServiceProvider: Send + Sync {
    fn call(&self, caller: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>>;
}

// ---------------------------------------------------------------------------
// 玩家动作
// ---------------------------------------------------------------------------

/// 玩家动作执行端：把 [`PlayerAction`] 应用到目标在线玩家。
/// 目标玩家不在线时静默忽略（v4 语义）。
// ---------------------------------------------------------------------------
// 插件命令注册
// ---------------------------------------------------------------------------

/// 插件命令注册表：插件 on_load/on_enable 时声明自己响应的顶层指令。
/// 客户端执行对应命令时宿主 fire [`ChatCommandEvent`]（v4 `commands` 事件
/// 语义）：cancel = 插件接管，不再进内置指令树。
pub trait CommandApi: Send + Sync {
    /// 声明顶层命令（如 `"menu"`）。重复声明幂等；随插件禁用自动清理。
    fn register_command(&self, plugin_id: &str, command: &str);
    /// 当前全部已注册的插件顶层命令（快照，供补全/帮助用）。
    fn plugin_commands(&self) -> Vec<String>;
}

pub trait PlayerActionApi: Send + Sync {
    /// 对指定玩家执行动作序列。返回实际执行的条数。
    fn apply_actions(&self, username: &str, actions: &[PlayerAction]) -> usize;
    /// 全部在线玩家用户名（快照）。
    fn online_players(&self) -> Vec<String>;
}
