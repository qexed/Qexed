use anyhow::Result;
use base64::Engine as _;
use bytes::BytesMut;
use qexed_config::app::qexed::server::Spawn;
use qexed_packet::net_types::GameProfile;
use qexed_packet::{PacketCodec, PacketReader, PacketWriter};
use qexed_protocol::to_client::play::{add_entity::EntityPosition, set_equipment::Equipment};
use serde::{Deserialize, Serialize};

use crate::inventory::PlayerInventory;
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
            inventory: PlayerInventory::empty().to_stored(),
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

    pub fn inventory(&self) -> PlayerInventory {
        PlayerInventory::from_stored(&self.inventory)
    }

    pub fn update_runtime(
        &mut self,
        dimension: &str,
        position: EntityPosition,
        inventory: &PlayerInventory,
        survival: StoredSurvival,
    ) {
        self.dimension = dimension.to_string();
        self.position = StoredPosition::from(position);
        self.survival = survival;
        self.inventory = inventory.to_stored();
    }

    pub fn raw_nbt_bytes(&self) -> Result<Option<Vec<u8>>> {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredEquipment {
    pub slot: u8,
    pub item: StoredSlot,
}

impl From<&Equipment> for StoredEquipment {
    fn from(value: &Equipment) -> Self {
        Self {
            slot: value.slot,
            item: StoredSlot::from(&value.item),
        }
    }
}

impl From<&StoredEquipment> for Equipment {
    fn from(value: &StoredEquipment) -> Self {
        Equipment::new(value.slot, (&value.item).into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredSlot {
    #[serde(default)]
    pub packet_data: String,
    #[serde(default, skip_serializing)]
    pub item_count: Option<i32>,
    #[serde(default, skip_serializing)]
    pub item_id: Option<i32>,
}

impl From<&qexed_protocol::types::Slot> for StoredSlot {
    fn from(value: &qexed_protocol::types::Slot) -> Self {
        match encode_slot(value) {
            Ok(packet_data) => Self {
                packet_data,
                item_count: None,
                item_id: None,
            },
            Err(err) => {
                log::warn!("failed to serialize player inventory slot, saving empty slot: {err:#}");
                Self {
                    packet_data: encode_slot(&crate::inventory::empty_slot()).unwrap_or_default(),
                    item_count: None,
                    item_id: None,
                }
            }
        }
    }
}

impl From<&StoredSlot> for qexed_protocol::types::Slot {
    fn from(value: &StoredSlot) -> Self {
        if value.packet_data.is_empty() {
            return legacy_slot(value);
        }

        match decode_slot(&value.packet_data) {
            Ok(slot) => slot,
            Err(err) => {
                log::warn!("failed to decode player inventory slot, loading empty slot: {err:#}");
                crate::inventory::empty_slot()
            }
        }
    }
}

fn legacy_slot(value: &StoredSlot) -> qexed_protocol::types::Slot {
    let item_count = value.item_count.unwrap_or_default();
    if item_count <= 0 {
        return crate::inventory::empty_slot();
    }
    crate::inventory::simple_item(value.item_id.unwrap_or_default(), item_count)
}

fn encode_slot(slot: &qexed_protocol::types::Slot) -> Result<String> {
    let mut bytes = BytesMut::new();
    slot.serialize(&mut PacketWriter::new(&mut bytes))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn decode_slot(value: &str) -> Result<qexed_protocol::types::Slot> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(value)?;
    let mut slice = bytes.as_slice();
    let mut reader = PacketReader::new(&mut slice);
    reader.deserialize()
}
