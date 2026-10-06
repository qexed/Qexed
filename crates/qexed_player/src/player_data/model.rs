use base64::Engine as _;
use bytes::BytesMut;
use qexed_packet::net_types::GameProfile;
use qexed_packet::{PacketCodec, PacketReader, PacketWriter};
use qexed_protocol::types::{EntityPosition, Slot};
use serde::{Deserialize, Serialize};

use super::Spawn;

pub(super) const DATA_VERSION: i32 = 1;
const DEFAULT_HEALTH: f32 = 20.0;
const DEFAULT_FOOD: i32 = 20;
const DEFAULT_SATURATION: f32 = 5.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerData {
    pub data_version: i32,
    pub uuid: uuid::Uuid,
    pub profile_name: String,
    pub dimension: String,
    pub position: StoredPosition,
    #[serde(default)]
    pub survival: StoredSurvival,
    pub inventory: StoredInventory,
    #[serde(default)]
    pub raw_nbt: String,
}

impl PlayerData {
    pub fn from_spawn(profile: &GameProfile, dimension: &str, spawn: &Spawn) -> Self {
        Self {
            data_version: DATA_VERSION,
            uuid: profile.uuid,
            profile_name: profile.username.clone(),
            dimension: dimension.to_string(),
            position: StoredPosition {
                x: spawn.x,
                y: spawn.y,
                z: spawn.z,
                yaw: spawn.yaw,
                pitch: spawn.pitch,
                on_ground: true,
            },
            survival: StoredSurvival::default(),
            inventory: StoredInventory::default(),
            raw_nbt: String::new(),
        }
    }

    pub fn entity_position(&self) -> EntityPosition {
        EntityPosition {
            x: self.position.x,
            y: self.position.y,
            z: self.position.z,
            yaw: self.position.yaw,
            pitch: self.position.pitch,
            on_ground: self.position.on_ground,
        }
    }

    pub fn update_runtime(
        &mut self,
        dimension: &str,
        position: EntityPosition,
        inventory: &StoredInventory,
        survival: StoredSurvival,
    ) {
        self.dimension = dimension.to_string();
        self.position = StoredPosition::from(position);
        self.survival = survival;
        self.inventory = inventory.clone();
    }

    pub fn raw_nbt_bytes(&self) -> Result<Option<Vec<u8>>, base64::DecodeError> {
        if self.raw_nbt.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            base64::engine::general_purpose::STANDARD.decode(&self.raw_nbt)?,
        ))
    }

    pub fn set_raw_nbt_bytes(&mut self, bytes: &[u8]) {
        self.raw_nbt = base64::engine::general_purpose::STANDARD.encode(bytes);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StoredSurvival {
    #[serde(default = "default_health")]
    pub health: f32,
    #[serde(default = "default_food")]
    pub food: i32,
    #[serde(default = "default_saturation")]
    pub saturation: f32,
}

impl Default for StoredSurvival {
    fn default() -> Self {
        Self {
            health: default_health(),
            food: default_food(),
            saturation: default_saturation(),
        }
    }
}

fn default_health() -> f32 {
    DEFAULT_HEALTH
}

fn default_food() -> i32 {
    DEFAULT_FOOD
}

fn default_saturation() -> f32 {
    DEFAULT_SATURATION
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StoredPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl From<EntityPosition> for StoredPosition {
    fn from(value: EntityPosition) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
            yaw: value.yaw,
            pitch: value.pitch,
            on_ground: value.on_ground,
        }
    }
}

/// 存储用背包（v4 由 qexed 本体 inventory.rs 的 PlayerInventory::to_stored 产出；
/// v6 中背包运行时归 qexed_play，序列化格式保持一致以便互通）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredInventory {
    pub selected: usize,
    #[serde(default)]
    pub main: Vec<StoredSlot>,
    #[serde(default)]
    pub hotbar: Vec<StoredSlot>,
    #[serde(default)]
    pub equipment: Vec<StoredEquipment>,
}

impl Default for StoredInventory {
    /// v4 语义：空背包 = 27 main + 9 hotbar 空槽 + 6 空装备位。
    /// 这里以空槽列表显式展开，等价于 PlayerInventory::empty().to_stored()。
    fn default() -> Self {
        Self {
            selected: 0,
            main: vec![StoredSlot::default(); 27],
            hotbar: vec![StoredSlot::default(); 9],
            equipment: (0..6)
                .map(|slot| StoredEquipment {
                    slot,
                    item: StoredSlot::default(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredEquipment {
    pub slot: u8,
    pub item: StoredSlot,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StoredSlot {
    #[serde(default)]
    pub packet_data: String,
    #[serde(default, skip_serializing)]
    pub item_count: Option<i32>,
    #[serde(default, skip_serializing)]
    pub item_id: Option<i32>,
}

impl From<&Slot> for StoredSlot {
    fn from(value: &Slot) -> Self {
        match encode_slot(value) {
            Ok(packet_data) => Self {
                packet_data,
                item_count: None,
                item_id: None,
            },
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.player.data.slot_encode_failed")
                        .replace("%{error}", &format!("{err:#}"))
                );
                Self {
                    packet_data: encode_slot(&empty_slot()).unwrap_or_default(),
                    item_count: None,
                    item_id: None,
                }
            }
        }
    }
}

impl From<&StoredSlot> for Slot {
    fn from(value: &StoredSlot) -> Self {
        if value.packet_data.is_empty() {
            return legacy_slot(value);
        }

        match decode_slot(&value.packet_data) {
            Ok(slot) => slot,
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.player.data.slot_decode_failed")
                        .replace("%{error}", &format!("{err:#}"))
                );
                empty_slot()
            }
        }
    }
}

/// 数据库存储用的结构化 payload：raw_nbt 单独存列/字段，不进 JSON。
pub(super) fn structured_payload(data: &PlayerData) -> PlayerData {
    let mut payload = data.clone();
    payload.raw_nbt.clear();
    payload
}

fn legacy_slot(value: &StoredSlot) -> Slot {
    let item_count = value.item_count.unwrap_or_default();
    if item_count <= 0 {
        return empty_slot();
    }
    simple_item(value.item_id.unwrap_or_default(), item_count)
}

/// v4 crate::inventory::empty_slot 的本地等价（v6 背包本体在 qexed_play）。
pub(crate) fn empty_slot() -> Slot {
    Slot {
        item_count: qexed_packet::net_types::VarInt(0),
        item_id: None,
        number_of_components_to_add: None,
        number_of_components_to_remove: None,
        components_to_add: None,
        components_to_remove: None,
    }
}

/// v4 crate::inventory::simple_item 的本地等价。
pub(crate) fn simple_item(item_id: i32, count: i32) -> Slot {
    Slot {
        item_count: qexed_packet::net_types::VarInt(count),
        item_id: Some(qexed_packet::net_types::VarInt(item_id)),
        number_of_components_to_add: Some(qexed_packet::net_types::VarInt(0)),
        number_of_components_to_remove: Some(qexed_packet::net_types::VarInt(0)),
        components_to_add: None,
        components_to_remove: None,
    }
}

fn encode_slot(slot: &Slot) -> Result<String, qexed_packet::PacketError> {
    let mut bytes = BytesMut::new();
    slot.serialize(&mut PacketWriter::new(&mut bytes))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn decode_slot(value: &str) -> Result<Slot, qexed_packet::PacketError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|err| qexed_packet::PacketError::msg(format!("base64: {err}")))?;
    let mut slice = bytes.as_slice();
    let mut reader = PacketReader::new(&mut slice);
    reader.deserialize()
}