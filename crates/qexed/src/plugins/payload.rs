use serde::{Deserialize, Serialize};

use crate::players::OnlinePlayer;

#[derive(Serialize)]
pub(super) struct PlayerPayload<'a> {
    uuid: String,
    username: &'a str,
    entity_id: i32,
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

pub(super) fn player_payload(player: &OnlinePlayer) -> PlayerPayload<'_> {
    PlayerPayload {
        uuid: player.profile.uuid.to_string(),
        username: &player.profile.username,
        entity_id: player.entity_id,
    }
}

fn one() -> i32 {
    1
}
