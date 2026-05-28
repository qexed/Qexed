use serde::{Deserialize, Serialize};

use crate::players::OnlinePlayer;

#[derive(Serialize)]
pub(super) struct PlayerPayload<'a> {
    pub(super) uuid: String,
    pub(super) username: &'a str,
    pub(super) entity_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPayloadOwned {
    pub uuid: String,
    pub username: String,
    pub entity_id: i32,
}

#[derive(Serialize)]
pub(super) struct ChunkPayload<'a> {
    pub(super) dimension: &'a str,
    pub(super) chunk_x: i32,
    pub(super) chunk_z: i32,
}

#[derive(Serialize)]
pub(super) struct ConfigReloadPayload<'a> {
    pub(super) path: &'a str,
}

#[derive(Serialize)]
pub(super) struct LanguagePayload<'a> {
    pub(super) language: &'a str,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommandResponse {
    #[serde(default)]
    pub handled: bool,
    #[serde(default)]
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcInteractPayload {
    pub player: PlayerPayloadOwned,
    pub entity: NpcEntityPayload,
    pub action: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcMutationResponse {
    #[serde(default)]
    pub operations: Vec<NpcMutationOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
}

pub(super) fn player_payload(player: &OnlinePlayer) -> PlayerPayload<'_> {
    PlayerPayload {
        uuid: player.profile.uuid.to_string(),
        username: &player.profile.username,
        entity_id: player.entity_id,
    }
}

pub(super) fn player_payload_owned(player: &OnlinePlayer) -> PlayerPayloadOwned {
    PlayerPayloadOwned {
        uuid: player.profile.uuid.to_string(),
        username: player.profile.username.clone(),
        entity_id: player.entity_id,
    }
}

fn one() -> i32 {
    1
}

fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}
