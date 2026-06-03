use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStackPayload {
    pub item_id: i32,
    #[serde(default)]
    pub item_name: String,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
pub struct PlayerMovePayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub previous_position: PlayerPositionPayload,
    pub position: PlayerPositionPayload,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerInputState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputPayload {
    pub player: PlayerPayloadOwned,
    pub dimension: String,
    pub position: PlayerPositionPayload,
    pub previous_input: PlayerInputState,
    pub input: PlayerInputState,
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
    SetPlayersVisible {
        visible: bool,
    },
    Velocity {
        x: f64,
        y: f64,
        z: f64,
        #[serde(default)]
        additive: bool,
    },
}

#[cfg(feature = "server")]
pub fn player_payload(player: &qexed_player::OnlinePlayer) -> PlayerPayload {
    PlayerPayload {
        uuid: player.profile.uuid.to_string(),
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
        language: player.language.clone(),
        dimension: player.dimension.clone(),
    }
}

#[cfg(feature = "server")]
pub fn player_payload_owned(player: &qexed_player::OnlinePlayer) -> PlayerPayloadOwned {
    PlayerPayloadOwned {
        uuid: player.profile.uuid.to_string(),
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
        language: player.language.clone(),
        dimension: player.dimension.clone(),
    }
}

#[cfg(feature = "server")]
pub fn player_position_payload(
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
) -> PlayerPositionPayload {
    PlayerPositionPayload {
        x: position.x,
        y: position.y,
        z: position.z,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: position.on_ground,
    }
}

#[cfg(feature = "server")]
pub fn player_input_state(flags: u8) -> PlayerInputState {
    PlayerInputState {
        forward: flags & qexed_protocol::to_server::play::player_input::FORWARD != 0,
        backward: flags & qexed_protocol::to_server::play::player_input::BACKWARD != 0,
        left: flags & qexed_protocol::to_server::play::player_input::LEFT != 0,
        right: flags & qexed_protocol::to_server::play::player_input::RIGHT != 0,
        jump: flags & qexed_protocol::to_server::play::player_input::JUMP != 0,
        shift: flags & qexed_protocol::to_server::play::player_input::SHIFT != 0,
        sprint: flags & qexed_protocol::to_server::play::player_input::SPRINT != 0,
    }
}

fn default_dimension() -> String {
    "minecraft:overworld".to_string()
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
