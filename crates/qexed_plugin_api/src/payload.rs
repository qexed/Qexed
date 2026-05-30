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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDropItem {
    pub item_id: i32,
    #[serde(default = "one")]
    pub count: i32,
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
