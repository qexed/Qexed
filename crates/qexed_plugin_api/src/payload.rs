//! v4 插件契约的载荷类型全集（自 v4 `payload.rs` 逐字移植）。
//!
//! 字段名、serde 默认值与 v4 WASM 时代完全一致：旧插件的移植代码不需要
//! 改任何载荷构造。v6 中这些类型不再跨 `(ptr,len)` 序列化，而是直接以
//! 强类型在宿主与插件间传递（serde 派生保留，供插件自行持久化）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Serde helper: serializes BTreeMap<String, Value> as a JSON string (postcard-compatible)
mod ai_params_serde {
    use std::collections::BTreeMap;

    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        params: &BTreeMap<String, serde_json::Value>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let json = serde_json::to_string(params).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<String, serde_json::Value>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json: String = serde::Deserialize::deserialize(deserializer)?;
        serde_json::from_str(&json).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPayload {
    pub uuid: String,
    pub username: String,
    pub entity_id: i32,
    #[serde(default)]
    pub language: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPayloadOwned {
    pub uuid: String,
    pub username: String,
    pub entity_id: i32,
    #[serde(default)]
    pub language: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPayload {
    pub dimension: String,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReloadPayload {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePayload {
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemEnchantment {
    pub id: String,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginEnchantment {
    pub id: String,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningSpeedQuery {
    pub block_state: i32,
    pub block_name: String,
    pub item_id: Option<i32>,
    pub enchantments: Vec<ItemEnchantment>,
    pub plugin_enchantments: Vec<PluginEnchantment>,
    pub speed: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MiningSpeedResponse {
    pub speed: Option<f32>,
    pub multiplier: Option<f32>,
    pub add: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BlockDropPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDropQuery {
    #[serde(default)]
    pub player: Option<PlayerPayloadOwned>,
    #[serde(default)]
    pub player_position: Option<PlayerPositionPayload>,
    pub block_state: i32,
    pub block_name: String,
    pub position: BlockDropPosition,
    pub tool_item_id: Option<i32>,
    pub enchantments: Vec<ItemEnchantment>,
    pub plugin_enchantments: Vec<PluginEnchantment>,
    pub default_item_id: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockDropResponse {
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub items: Vec<BlockDropItem>,
    #[serde(default)]
    pub break_positions: Vec<BlockDropPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDropItem {
    pub item_id: i32,
    #[serde(default)]
    pub item_name: String,
    #[serde(default = "one")]
    pub count: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemStackPayload {
    pub item_id: i32,
    #[serde(default)]
    pub item_name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "one")]
    pub count: i32,
    #[serde(default)]
    pub damage: i32,
    #[serde(default)]
    pub max_damage: i32,
    #[serde(default)]
    pub enchantments: Vec<ItemEnchantment>,
    #[serde(default)]
    pub plugin_enchantments: Vec<PluginEnchantment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingRecipeQuery {
    pub player: PlayerPayloadOwned,
    pub width: usize,
    pub height: usize,
    #[serde(default)]
    pub ingredients: Vec<ItemStackPayload>,
    #[serde(default)]
    pub vanilla_result: Option<ItemStackPayload>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CraftingRecipeResponse {
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub result: Option<BlockDropItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftItemQuery {
    pub player: PlayerPayloadOwned,
    pub recipe_id: String,
    pub result: ItemStackPayload,
    #[serde(default)]
    pub ingredients: Vec<ItemStackPayload>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CraftItemResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub result: Option<BlockDropItem>,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantingQuery {
    pub player: PlayerPayloadOwned,
    pub item: ItemStackPayload,
    #[serde(default)]
    pub lapis_item: Option<ItemStackPayload>,
    pub level: i32,
    #[serde(default)]
    pub bookshelf_count: i32,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnchantingResponse {
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub options: Vec<EnchantingOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnchantingOption {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub level: i32,
    #[serde(default)]
    pub weight: i32,
    #[serde(default)]
    pub required_level: i32,
    #[serde(default)]
    pub lapis_cost: i32,
    #[serde(default)]
    pub item_damage_cost: i32,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FurnaceRecipeQuery {
    pub player: PlayerPayloadOwned,
    pub input: ItemStackPayload,
    pub fuel: ItemStackPayload,
    #[serde(default)]
    pub vanilla_result: Option<ItemStackPayload>,
    #[serde(default)]
    pub cook_time: i32,
    #[serde(default)]
    pub experience: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FurnaceRecipeResponse {
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub result: Option<BlockDropItem>,
    #[serde(default)]
    pub cook_time: Option<i32>,
    #[serde(default)]
    pub experience: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FurnaceTickPayload {
    pub player: PlayerPayloadOwned,
    pub input: Option<ItemStackPayload>,
    pub fuel: Option<ItemStackPayload>,
    pub output: Option<ItemStackPayload>,
    pub burn_time: i32,
    pub cook_time: i32,
    pub cook_time_total: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDurabilityQuery {
    pub player: PlayerPayloadOwned,
    pub item: ItemStackPayload,
    pub reason: String,
    #[serde(default = "one")]
    pub amount: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemDurabilityResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub amount: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAttackQuery {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub target_entity_id: i32,
    #[serde(default)]
    pub target_uuid: [u8; 16],
    #[serde(default)]
    pub target_player: Option<PlayerPayloadOwned>,
    #[serde(default)]
    pub target_type: String,
    pub weapon: ItemStackPayload,
    pub damage: f32,
    pub knockback: f32,
    #[serde(default)]
    pub fire_ticks: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerAttackResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub damage: Option<f32>,
    #[serde(default)]
    pub knockback: Option<f32>,
    #[serde(default)]
    pub fire_ticks: Option<i32>,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerOxygenTickQuery {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub air: i32,
    pub max_air: i32,
    pub underwater: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerOxygenTickResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub air: Option<i32>,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundPayload {
    pub player: Option<PlayerPayloadOwned>,
    pub sound: String,
    pub source: String,
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub volume: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoundResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub volume: Option<f32>,
    #[serde(default)]
    pub pitch: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancementGrantQuery {
    pub player: PlayerPayloadOwned,
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdvancementGrantResponse {
    #[serde(default)]
    pub cancel: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotionEffectTickQuery {
    pub player: PlayerPayloadOwned,
    pub effect: String,
    pub amplifier: i32,
    pub duration_ticks: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PotionEffectTickResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub duration_ticks: Option<i32>,
    #[serde(default)]
    pub amplifier: Option<i32>,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BlockStepPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PlayerPositionPayload {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStepPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub block_state: i32,
    pub block_name: String,
    pub position: BlockStepPosition,
    pub player_position: PlayerPositionPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerBlockInteractPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub block_state: i32,
    pub block_name: String,
    pub position: BlockDropPosition,
    pub player_position: PlayerPositionPayload,
    #[serde(default)]
    pub hand: String,
    #[serde(default)]
    pub sequence: i32,
    #[serde(default)]
    pub face: String,
    #[serde(default)]
    pub face_id: i32,
    #[serde(default)]
    pub cursor_x: f32,
    #[serde(default)]
    pub cursor_y: f32,
    #[serde(default)]
    pub cursor_z: f32,
    #[serde(default)]
    pub inside_block: bool,
    #[serde(default)]
    pub world_border_hit: bool,
    #[serde(default)]
    pub input: PlayerInputState,
    #[serde(default)]
    pub client: PlayerClientPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMovePayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub previous_position: PlayerPositionPayload,
    pub position: PlayerPositionPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerTickPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub tick_millis: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInputState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
    /// 玩家贴地标志（与位置包分开维护，事件侧读取）。
    pub on_ground: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerClientPayload {
    #[serde(default)]
    pub bedrock: bool,
    #[serde(default)]
    pub floodgate: bool,
    #[serde(default)]
    pub xuid: String,
    #[serde(default)]
    pub device_os: String,
    #[serde(default)]
    pub input_mode: String,
    #[serde(default)]
    pub ui_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub previous_input: PlayerInputState,
    pub input: PlayerInputState,
    #[serde(default)]
    pub client: PlayerClientPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerUseItemPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub hand: String,
    pub action: String,
    pub item: ItemStackPayload,
    #[serde(default)]
    pub sequence: i32,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub input: PlayerInputState,
    #[serde(default)]
    pub client: PlayerClientPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectileHitPlayerPayload {
    pub shooter: PlayerPayloadOwned,
    pub target: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    #[serde(default)]
    pub projectile_entity_id: i32,
    #[serde(default)]
    pub projectile_kind: String,
    #[serde(default)]
    pub configured_event: String,
    #[serde(default)]
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickDetectedPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub action: String,
    pub clicks: u32,
    pub window_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerItemPickupQuery {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub item_entity_id: i32,
    #[serde(default)]
    pub item_id: Option<i32>,
    #[serde(default)]
    pub item_name: String,
    pub count: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerItemPickupResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub consume: bool,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerDeathQuery {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub cause: String,
    #[serde(default)]
    pub source_entity_id: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerDeathResponse {
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub overlay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathfindingQuery {
    pub dimension: String,
    pub start: BlockDropPosition,
    pub goal: BlockDropPosition,
    #[serde(default)]
    pub max_nodes: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PathfindingResponse {
    #[serde(default)]
    pub found: bool,
    #[serde(default)]
    pub path: Vec<BlockDropPosition>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginCommandDefinition {
    pub name: String,
    #[serde(default)]
    pub description_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommandQuery {
    pub command: String,
    pub argument: String,
    pub player: PlayerPayloadOwned,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginCommandResponse {
    #[serde(default)]
    pub handled: bool,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    #[serde(default)]
    pub version: String,
    /// Hard dependencies: plugin will be disabled if any of these are missing.
    /// The load order is determined by dependency graph topological sort.
    #[serde(default)]
    pub depends: Vec<PluginDependency>,
    /// Optional dependencies: loaded first if available, but plugin is not
    /// disabled when they are missing.
    #[serde(default)]
    pub optional_depends: Vec<PluginDependency>,
    /// Soft ordering: load after the listed plugins (without version constraint).
    /// Missing plugins are silently ignored. Used when a plugin wants to run
    /// after another plugin but doesn't strictly depend on it.
    #[serde(default)]
    pub load_after: Vec<String>,
    #[serde(default)]
    pub services: Vec<PluginServiceDefinition>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginDependency {
    pub id: String,
    #[serde(default)]
    pub version: String,
    /// Ordering priority within the dependency layer. Higher values mean the
    /// dependency should be loaded earlier relative to other dependencies at
    /// the same depth. Defaults to 0.
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginServiceDefinition {
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginApiCallQuery {
    pub service: String,
    pub method: String,
    #[serde(default)]
    pub payload: Vec<u8>,
    #[serde(default)]
    pub caller: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginApiCallResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub payload: Vec<u8>,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeyserPlayerInfoQuery {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub username: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeyserPlayerInfoResponse {
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub bedrock: bool,
    #[serde(default)]
    pub floodgate: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub java_uuid: String,
    #[serde(default)]
    pub xuid: String,
    #[serde(default)]
    pub device_os: String,
    #[serde(default)]
    pub input_mode: String,
    #[serde(default)]
    pub ui_profile: String,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockFormResponsePayload {
    pub player: PlayerPayloadOwned,
    #[serde(default)]
    pub form_id: u16,
    #[serde(default)]
    pub plugin_form_id: String,
    #[serde(default)]
    pub response: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRequest {
    #[serde(default = "default_http_method")]
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
    #[serde(default)]
    pub body: Vec<u8>,
    #[serde(default)]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
    #[serde(default)]
    pub body: Vec<u8>,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConnectResultPayload {
    pub player: PlayerPayloadOwned,
    pub target_server: String,
    #[serde(default)]
    pub current_server: String,
    #[serde(default)]
    pub proxy_protocol: String,
    pub status_code: i32,
    pub status: String,
    pub success: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderQuery {
    pub player: Option<PlayerPayloadOwned>,
    pub text: String,
    #[serde(default)]
    pub context: Vec<PlaceholderContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderContext {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaceholderResponse {
    #[serde(default)]
    pub replacements: Vec<PlaceholderReplacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderReplacement {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcInteractPayload {
    pub player: PlayerPayloadOwned,
    pub entity: NpcEntityPayload,
    pub action: String,
    #[serde(default)]
    pub hand: String,
    #[serde(default)]
    pub configured_event: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcEntityPayload {
    pub key: String,
    pub entity_id: i32,
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NpcPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcMove {
    pub key: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    pub position: NpcPosition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NpcPatrolPoint {
    pub position: NpcPosition,
    #[serde(default)]
    pub wait_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcPatrol {
    pub key: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    #[serde(default)]
    pub points: Vec<NpcPatrolPoint>,
    #[serde(default = "default_npc_patrol_speed")]
    pub speed_blocks_per_second: f64,
    #[serde(default)]
    pub looped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcMutationQuery {
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NpcMutationResponse {
    #[serde(default)]
    pub operations: Vec<NpcMutationOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NpcMutationOp {
    Upsert { npc: NpcUpsert },
    Remove { key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcUpsert {
    pub key: String,
    #[serde(default = "default_dimension")]
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub entity_type: String,
    #[serde(default)]
    pub skin_textures: String,
    #[serde(default)]
    pub skin_signature: String,
    #[serde(default)]
    pub look_at_players: bool,
    #[serde(default = "default_npc_main_hand_event")]
    pub main_hand_event: String,
    #[serde(default = "default_npc_off_hand_event")]
    pub off_hand_event: String,
    #[serde(default = "default_npc_attack_event")]
    pub attack_event: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomEntityDefinition {
    pub id: String,
    #[serde(default)]
    pub entity_type: String,
    #[serde(default)]
    pub shell_entity_type: String,
    #[serde(default)]
    pub registry_id: Option<i32>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub ai: String,
    #[serde(default)]
    #[serde(with = "ai_params_serde")]
    pub ai_params: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomEntityRegistryResponse {
    #[serde(default)]
    pub entities: Vec<CustomEntityDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAiTickQuery {
    pub entity: EntityAiEntityPayload,
    #[serde(default)]
    pub nearby_players: Vec<EntityAiPlayerPayload>,
    pub tick_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAiEntityPayload {
    pub key: String,
    pub entity_id: i32,
    pub entity_type: String,
    #[serde(default)]
    pub custom_type: String,
    #[serde(default)]
    pub ai: String,
    #[serde(default)]
    pub spawn_rule: String,
    #[serde(default)]
    #[serde(with = "ai_params_serde")]
    pub ai_params: BTreeMap<String, serde_json::Value>,
    pub dimension: String,
    pub position: PlayerPositionPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAiPlayerPayload {
    pub player: PlayerPayloadOwned,
    pub position: PlayerPositionPayload,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityAiTickResponse {
    #[serde(default)]
    pub operations: Vec<EntityAiOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityAiOperation {
    MoveDelta {
        x: f64,
        y: f64,
        z: f64,
        #[serde(default)]
        yaw: Option<f32>,
        #[serde(default)]
        pitch: Option<f32>,
    },
    LookAt {
        x: f64,
        y: f64,
        z: f64,
    },
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedNameQuery {
    pub key: String,
    pub language: String,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalizedNameResponse {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerAction {
    SystemMessage {
        #[serde(default)]
        text: String,
        #[serde(default)]
        translate: String,
        #[serde(default)]
        with: Vec<String>,
        #[serde(default)]
        overlay: bool,
    },
    Teleport {
        #[serde(default)]
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        #[serde(default)]
        yaw: Option<f32>,
        #[serde(default)]
        pitch: Option<f32>,
    },
    Transfer {
        host: String,
        port: u16,
        #[serde(default)]
        message: String,
    },
    ProxyConnect {
        server: String,
        #[serde(default)]
        message: String,
    },
    OpenMenu {
        menu: String,
    },
    GiveItem {
        item: String,
        #[serde(default = "one")]
        count: i32,
        #[serde(default)]
        name: String,
        #[serde(default)]
        lore: Vec<String>,
        #[serde(default)]
        enchantments: Vec<ItemEnchantment>,
        #[serde(default)]
        plugin_enchantments: Vec<PluginEnchantment>,
    },
    ResetInventory {
        #[serde(default)]
        restore_menu_items: bool,
    },
    SetPlayersVisible {
        visible: bool,
    },
    SpawnProjectile {
        #[serde(default)]
        kind: String,
        #[serde(default)]
        configured_event: String,
        #[serde(default)]
        tag: String,
        #[serde(default)]
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        velocity_x: f64,
        velocity_y: f64,
        velocity_z: f64,
        #[serde(default)]
        source_entity_id: i32,
        #[serde(default)]
        damage: f32,
        #[serde(default)]
        knockback: f32,
        #[serde(default)]
        gravity_per_tick: f64,
        #[serde(default)]
        hit_radius: f64,
        #[serde(default)]
        lifetime_ticks: i32,
    },
    Velocity {
        x: f64,
        y: f64,
        z: f64,
        #[serde(default)]
        additive: bool,
    },
    DamagePlayer {
        #[serde(default)]
        uuid: String,
        #[serde(default)]
        username: String,
        amount: f32,
        #[serde(default)]
        kind: String,
        #[serde(default)]
        source_entity_id: i32,
        #[serde(default)]
        source_x: f64,
        #[serde(default)]
        source_y: f64,
        #[serde(default)]
        source_z: f64,
        #[serde(default)]
        knockback: f32,
    },
    BossBar {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        progress: f32,
        #[serde(default)]
        color: String,
        #[serde(default)]
        overlay: String,
    },
    RemoveBossBar {
        id: String,
    },
    SendBedrockForm {
        #[serde(default)]
        uuid: String,
        #[serde(default)]
        username: String,
        #[serde(default)]
        form_id: Option<u16>,
        #[serde(default)]
        plugin_form_id: String,
        #[serde(default)]
        form_type: String,
        json: String,
    },
}

fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_http_method() -> String {
    "GET".to_string()
}

fn one() -> i32 {
    1
}

fn default_npc_main_hand_event() -> String {
    "interact".to_string()
}

fn default_npc_off_hand_event() -> String {
    "interact_off_hand".to_string()
}

fn default_npc_attack_event() -> String {
    "attack".to_string()
}

fn default_npc_patrol_speed() -> f64 {
    2.0
}


// ── v6 便捷转换：PlayerInfo（v6 玩家快照）→ v4 载荷 ──

impl PlayerPayload {
    /// 从 v6 的 [`crate::PlayerInfo`] 构造（language 未知，填空串）。
    pub fn from_info(info: &crate::PlayerInfo) -> Self {
        Self {
            uuid: info.uuid.clone(),
            username: info.username.clone(),
            entity_id: info.entity_id,
            language: String::new(),
            dimension: info.dimension.clone(),
        }
    }
}

impl PlayerPayloadOwned {
    /// 从 v6 的 [`crate::PlayerInfo`] 构造（language 未知，填空串）。
    pub fn from_info(info: &crate::PlayerInfo) -> Self {
        Self {
            uuid: info.uuid.clone(),
            username: info.username.clone(),
            entity_id: info.entity_id,
            language: String::new(),
            dimension: info.dimension.clone(),
        }
    }
}

impl PlayerPositionPayload {
    pub fn from_parts(x: f64, y: f64, z: f64, yaw: f32, pitch: f32, on_ground: bool) -> Self {
        Self { x, y, z, yaw, pitch, on_ground }
    }
}
