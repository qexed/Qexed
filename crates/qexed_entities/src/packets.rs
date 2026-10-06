//! 实体数据包组装（v4 qexed_entity::packets → v6 协议）。
//!
//! v6 协议差异（相对 v4）：
//! - `AddEntity`：扁平字段 + `LpVec3 movement`（26.3 压缩向量）、角度为 256 级字节，
//!   无 `new/player` 构造器 → 本地实现 `build_add_entity`；
//! - `SetEntityMotion`：字段名 movement_x/y/z，无 `from_velocity`；
//! - `EntityPositionSync`：PositionPath 编码 → 本地构造 Linear 路径；
//! - `RotateHead`：y_head_rot 为 i8 角度字节；
//! - `RemoveEntities`/`PlayerInfoRemove`：Vec 字段，无 one() 帮助函数。

use bytes::{Bytes, BytesMut};
use qexed_packet::{
    Packet, PacketCodec,
    net_types::{GameProfile, ProfileProperty, VarInt},
};
use qexed_protocol::{
    to_client::play::{
        add_entity::{AddEntity, LpVec3},
        entity_position_sync::{EntityPositionSync, PositionPath},
        player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
        remove_entities::RemoveEntities,
        rotate_head::RotateHead,
        set_entity_data::SetEntityData,
        set_entity_motion::SetEntityMotion,
        take_item_entity::TakeItemEntity,
    },
    types::{EntityMetadata, EntityMetadataEnum, EntityMetadataSub, Slot, TextComponent},
};

use std::{collections::HashMap, sync::Arc};

use crate::{
    DroppedItemEntity, ManagedEntity, ManagedEntityKind, error::EntitiesError, position::EntityPosition,
};

const ITEM_ENTITY_METADATA_ITEM_INDEX: u8 = 8;
const DISPLAY_BILLBOARD_METADATA_INDEX: u8 = 15;
const DISPLAY_VIEW_RANGE_METADATA_INDEX: u8 = 17;
const DISPLAY_WIDTH_METADATA_INDEX: u8 = 20;
const DISPLAY_HEIGHT_METADATA_INDEX: u8 = 21;
const TEXT_DISPLAY_TEXT_METADATA_INDEX: u8 = 23;
const TEXT_DISPLAY_LINE_WIDTH_METADATA_INDEX: u8 = 24;
const TEXT_DISPLAY_BACKGROUND_METADATA_INDEX: u8 = 25;
const TEXT_DISPLAY_OPACITY_METADATA_INDEX: u8 = 26;
const TEXT_DISPLAY_STYLE_METADATA_INDEX: u8 = 27;
const DISPLAY_BILLBOARD_CENTER: u8 = 3;
const TEXT_DISPLAY_DEFAULT_BACKGROUND: i32 = 0x40000000;
const TEXT_DISPLAY_DEFAULT_OPACITY: u8 = 0xff;
const TEXT_DISPLAY_SEE_THROUGH: u8 = 0x02;
const TEXT_DISPLAY_USE_DEFAULT_BACKGROUND: u8 = 0x04;
const ENTITY_CUSTOM_NAME_METADATA_INDEX: u8 = 2;
const ENTITY_CUSTOM_NAME_VISIBLE_METADATA_INDEX: u8 = 3;
const SLIME_SIZE_METADATA_INDEX: u8 = 16;
const DEFAULT_SLIME_SIZE: i32 = 4;

/// 角度（度）→ 256 级有符号字节。
fn pack_degrees(degrees: f32) -> i8 {
    let normalized = degrees.rem_euclid(360.0);
    ((normalized / 360.0) * 256.0).round() as i8
}

/// 构造 v6 AddEntity（无构造器，字段直填；movement 走 LpVec3 压缩）。
fn build_add_entity(
    entity_id: i32,
    uuid: uuid::Uuid,
    entity_type_id: i32,
    position: EntityPosition,
    data: i32,
    velocity: (f64, f64, f64),
) -> AddEntity {
    AddEntity {
        id: VarInt(entity_id),
        uuid,
        entity_type: VarInt(entity_type_id),
        x: position.x,
        y: position.y,
        z: position.z,
        movement: LpVec3 {
            x: velocity.0,
            y: velocity.1,
            z: velocity.2,
        },
        x_rot: pack_degrees(position.pitch),
        y_rot: pack_degrees(position.yaw),
        y_head_rot: pack_degrees(position.yaw),
        data: VarInt(data),
    }
}

/// 构造 v6 SetEntityMotion。
fn build_entity_motion(
    entity_id: i32,
    velocity_x: f64,
    velocity_y: f64,
    velocity_z: f64,
) -> SetEntityMotion {
    SetEntityMotion {
        id: VarInt(entity_id),
        movement_x: velocity_x,
        movement_y: velocity_y,
        movement_z: velocity_z,
    }
}

impl ManagedEntity {
    pub fn spawn_packets(&self) -> Result<Vec<Bytes>, EntitiesError> {
        self.spawn_packets_with_velocity(0.0, 0.0, 0.0)
    }

    pub fn spawn_packets_with_velocity(
        &self,
        velocity_x: f64,
        velocity_y: f64,
        velocity_z: f64,
    ) -> Result<Vec<Bytes>, EntitiesError> {
        let mut packets = Vec::new();
        let player_npc =
            self.kind == ManagedEntityKind::Npc && self.entity_type == "minecraft:player";
        if player_npc {
            let display_name = self.display_name().map(text_component_or_json);
            packets.push(packet_bytes(PlayerInfoUpdate {
                actions: PlayerInfoActions::player_initializing(),
                entries: vec![npc_player_info_entry(&self.profile(), 0, display_name)],
            })?);
        }

        let add_entity = build_add_entity(
            self.entity_id,
            self.uuid,
            self.entity_type_id,
            self.position,
            self.data,
            (velocity_x, velocity_y, velocity_z),
        );
        packets.push(packet_bytes(add_entity)?);
        if velocity_x != 0.0 || velocity_y != 0.0 || velocity_z != 0.0 {
            packets.push(packet_bytes(build_entity_motion(
                self.entity_id,
                velocity_x,
                velocity_y,
                velocity_z,
            ))?);
        }
        packets.push(packet_bytes(RotateHead {
            entity_id: VarInt(self.entity_id),
            y_head_rot: pack_degrees(self.position.yaw),
        })?);

        let metadata = match self.kind {
            ManagedEntityKind::Hologram => Some(hologram_metadata(self.display_name())),
            ManagedEntityKind::Npc | ManagedEntityKind::Entity if player_npc => {
                Some(player_entity_metadata(self.display_name()))
            }
            ManagedEntityKind::Npc => self.display_name().map(named_entity_metadata),
            ManagedEntityKind::Entity => ordinary_entity_metadata(self),
        };
        if let Some(metadata) = metadata {
            packets.push(packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata,
            })?);
        }

        Ok(packets)
    }

    pub fn remove_packets(&self) -> Result<Vec<Bytes>, EntitiesError> {
        let mut packets = vec![packet_bytes(RemoveEntities {
            entity_ids: vec![VarInt(self.entity_id)],
        })?];
        if self.kind == ManagedEntityKind::Npc && self.entity_type == "minecraft:player" {
            packets.push(packet_bytes(
                qexed_protocol::to_client::play::player_info_remove::PlayerInfoRemove {
                    profile_ids: vec![self.uuid],
                },
            )?);
        }
        Ok(packets)
    }

    pub fn position_packets(&self) -> Result<Vec<Bytes>, EntitiesError> {
        self.position_packets_with_velocity(0.0, 0.0, 0.0)
    }

    pub fn position_packets_with_velocity(
        &self,
        velocity_x: f64,
        velocity_y: f64,
        velocity_z: f64,
    ) -> Result<Vec<Bytes>, EntitiesError> {
        let mut packets = vec![packet_bytes(EntityPositionSync {
            id: VarInt(self.entity_id),
            position: PositionPath::Linear {
                x: self.position.x,
                y: self.position.y,
                z: self.position.z,
            },
            y_rot: self.position.yaw,
            x_rot: self.position.pitch,
            on_ground: self.position.on_ground,
        })?];
        if velocity_x != 0.0 || velocity_y != 0.0 || velocity_z != 0.0 {
            packets.push(packet_bytes(build_entity_motion(
                self.entity_id,
                velocity_x,
                velocity_y,
                velocity_z,
            ))?);
        }
        packets.push(packet_bytes(RotateHead {
            entity_id: VarInt(self.entity_id),
            y_head_rot: pack_degrees(self.position.yaw),
        })?);
        Ok(packets)
    }

    fn profile(&self) -> GameProfile {
        let profile_name = self
            .display_name()
            .map(display_name_profile_text)
            .unwrap_or_else(|| self.name.clone());
        GameProfile {
            uuid: self.uuid,
            username: npc_profile_name(&profile_name),
            properties: self.skin_properties(),
        }
    }

    fn display_name(&self) -> Option<&str> {
        let name = self.display_name.trim();
        if !name.is_empty() {
            return Some(name);
        }
        if self.kind == ManagedEntityKind::Npc {
            Some(&self.key)
        } else {
            None
        }
    }

    fn skin_properties(&self) -> Vec<ProfileProperty> {
        let textures = self.skin_textures.trim();
        if textures.is_empty() {
            return Vec::new();
        }
        vec![ProfileProperty {
            name: "textures".to_string(),
            value: textures.to_string(),
            signature: {
                let signature = self.skin_signature.trim();
                if signature.is_empty() {
                    None
                } else {
                    Some(signature.to_string())
                }
            },
        }]
    }
}

impl DroppedItemEntity {
    pub fn spawn_packets(&self, entity_type_id: i32) -> Result<Vec<Bytes>, EntitiesError> {
        Ok(vec![
            packet_bytes(build_add_entity(
                self.entity_id,
                self.uuid,
                entity_type_id,
                self.position,
                0,
                (0.0, 0.0, 0.0),
            ))?,
            packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata: item_entity_metadata(self.item.clone()),
            })?,
        ])
    }

    pub fn pickup_packets(&self, collector_entity_id: i32) -> Result<Vec<Bytes>, EntitiesError> {
        let amount = self.item.item_count.0.max(1);
        Ok(vec![
            packet_bytes(TakeItemEntity {
                item_id: VarInt(self.entity_id),
                player_id: VarInt(collector_entity_id),
                amount: VarInt(amount),
            })?,
            packet_bytes(RemoveEntities {
                entity_ids: vec![VarInt(self.entity_id)],
            })?,
        ])
    }

    pub fn metadata_packets(&self) -> Result<Vec<Bytes>, EntitiesError> {
        Ok(vec![packet_bytes(SetEntityData {
            entity_id: VarInt(self.entity_id),
            metadata: item_entity_metadata(self.item.clone()),
        })?])
    }
}

fn hologram_metadata(text: Option<&str>) -> EntityMetadata {
    EntityMetadata {
        data: vec![
            EntityMetadataSub {
                index: DISPLAY_BILLBOARD_METADATA_INDEX,
                data: Some(EntityMetadataEnum::Byte(DISPLAY_BILLBOARD_CENTER)),
            },
            EntityMetadataSub {
                index: DISPLAY_VIEW_RANGE_METADATA_INDEX,
                data: Some(EntityMetadataEnum::Float(64.0)),
            },
            EntityMetadataSub {
                index: DISPLAY_WIDTH_METADATA_INDEX,
                data: Some(EntityMetadataEnum::Float(8.0)),
            },
            EntityMetadataSub {
                index: DISPLAY_HEIGHT_METADATA_INDEX,
                data: Some(EntityMetadataEnum::Float(1.0)),
            },
            EntityMetadataSub {
                index: TEXT_DISPLAY_TEXT_METADATA_INDEX,
                data: Some(EntityMetadataEnum::TextComponent(text_component_or_json(
                    text.unwrap_or_default(),
                ))),
            },
            EntityMetadataSub {
                index: TEXT_DISPLAY_LINE_WIDTH_METADATA_INDEX,
                data: Some(EntityMetadataEnum::VarInt(VarInt(200))),
            },
            EntityMetadataSub {
                index: TEXT_DISPLAY_BACKGROUND_METADATA_INDEX,
                data: Some(EntityMetadataEnum::VarInt(VarInt(
                    TEXT_DISPLAY_DEFAULT_BACKGROUND,
                ))),
            },
            EntityMetadataSub {
                index: TEXT_DISPLAY_OPACITY_METADATA_INDEX,
                data: Some(EntityMetadataEnum::Byte(TEXT_DISPLAY_DEFAULT_OPACITY)),
            },
            EntityMetadataSub {
                index: TEXT_DISPLAY_STYLE_METADATA_INDEX,
                data: Some(EntityMetadataEnum::Byte(
                    TEXT_DISPLAY_SEE_THROUGH | TEXT_DISPLAY_USE_DEFAULT_BACKGROUND,
                )),
            },
            EntityMetadataSub {
                index: 0xff,
                data: None,
            },
        ],
    }
}

fn npc_player_info_entry(
    profile: &GameProfile,
    game_mode: i32,
    display_name: Option<TextComponent>,
) -> PlayerInfoEntry {
    let mut entry = PlayerInfoEntry::from_profile(profile, game_mode);
    entry.listed = false;
    entry.display_name = display_name;
    entry
}

pub fn npc_profile_name(name: &str) -> String {
    let mut username = name
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(16)
        .collect::<String>();
    if username.is_empty() {
        username.push_str("NPC");
    }
    username
}

fn named_entity_metadata(name: &str) -> EntityMetadata {
    EntityMetadata {
        data: vec![
            EntityMetadataSub {
                index: 2,
                data: Some(EntityMetadataEnum::OptionTextComponent(Some(
                    text_component_or_json(name),
                ))),
            },
            EntityMetadataSub {
                index: 3,
                data: Some(EntityMetadataEnum::Boolean(true)),
            },
            EntityMetadataSub {
                index: 0xff,
                data: None,
            },
        ],
    }
}

fn player_entity_metadata(name: Option<&str>) -> EntityMetadata {
    let mut data = Vec::new();
    if let Some(name) = name {
        data.push(EntityMetadataSub {
            index: 2,
            data: Some(EntityMetadataEnum::OptionTextComponent(Some(
                text_component_or_json(name),
            ))),
        });
        data.push(EntityMetadataSub {
            index: 3,
            data: Some(EntityMetadataEnum::Boolean(true)),
        });
    }
    data.push(EntityMetadataSub {
        index: 0xff,
        data: None,
    });
    EntityMetadata { data }
}

fn ordinary_entity_metadata(entity: &ManagedEntity) -> Option<EntityMetadata> {
    let mut data = Vec::new();
    if let Some(name) = entity.display_name() {
        push_display_name_metadata(&mut data, name);
    }

    if matches!(
        entity.entity_type.as_str(),
        "minecraft:slime" | "minecraft:magma_cube"
    ) {
        push_slime_size_metadata(&mut data, slime_size_from_data(entity.data));
    }

    if data.is_empty() {
        return None;
    }

    push_metadata_end(&mut data);
    Some(EntityMetadata { data })
}

fn push_display_name_metadata(data: &mut Vec<EntityMetadataSub>, name: &str) {
    data.push(EntityMetadataSub {
        index: ENTITY_CUSTOM_NAME_METADATA_INDEX,
        data: Some(EntityMetadataEnum::OptionTextComponent(Some(
            text_component_or_json(name),
        ))),
    });
    data.push(EntityMetadataSub {
        index: ENTITY_CUSTOM_NAME_VISIBLE_METADATA_INDEX,
        data: Some(EntityMetadataEnum::Boolean(true)),
    });
}

fn push_slime_size_metadata(data: &mut Vec<EntityMetadataSub>, size: i32) {
    data.push(EntityMetadataSub {
        index: SLIME_SIZE_METADATA_INDEX,
        data: Some(EntityMetadataEnum::VarInt(VarInt(size.clamp(1, 127)))),
    });
}

fn push_metadata_end(data: &mut Vec<EntityMetadataSub>) {
    data.push(EntityMetadataSub {
        index: 0xff,
        data: None,
    });
}

fn slime_size_from_data(data: i32) -> i32 {
    if data > 0 {
        data.clamp(1, 127)
    } else {
        DEFAULT_SLIME_SIZE
    }
}

fn item_entity_metadata(item: Slot) -> EntityMetadata {
    EntityMetadata {
        data: vec![
            EntityMetadataSub {
                index: ITEM_ENTITY_METADATA_ITEM_INDEX,
                data: Some(EntityMetadataEnum::Slot(item)),
            },
            EntityMetadataSub {
                index: 0xff,
                data: None,
            },
        ],
    }
}

fn text_component(text: impl Into<String>) -> TextComponent {
    let mut map = HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(Arc::new(map))
}

fn text_component_or_json(text: &str) -> TextComponent {
    json_text_component(text).unwrap_or_else(|| text_component(text))
}

fn display_name_profile_text(text: &str) -> String {
    json_component_plain_text(text).unwrap_or_else(|| text.to_string())
}

fn json_component_plain_text(text: &str) -> Option<String> {
    let text = text.trim();
    if !(text.starts_with('{') || text.starts_with('[')) {
        return None;
    }

    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let mut output = String::new();
    append_component_plain_text(&value, &mut output);
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn append_component_plain_text(value: &serde_json::Value, output: &mut String) {
    match value {
        serde_json::Value::String(text) => output.push_str(text),
        serde_json::Value::Array(values) => {
            for value in values {
                append_component_plain_text(value, output);
            }
        }
        serde_json::Value::Object(values) => {
            if let Some(text) = values.get("text").and_then(serde_json::Value::as_str) {
                output.push_str(text);
            } else if let Some(translate) = values
                .get("translate")
                .and_then(serde_json::Value::as_str)
                .filter(|_| output.is_empty())
            {
                output.push_str(translate);
            }

            if let Some(extra) = values.get("extra") {
                append_component_plain_text(extra, output);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn json_text_component(text: &str) -> Option<TextComponent> {
    let text = text.trim();
    if !(text.starts_with('{') || text.starts_with('[')) {
        return None;
    }
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    Some(json_to_nbt(&value))
}

fn json_to_nbt(value: &serde_json::Value) -> qexed_nbt::Tag {
    match value {
        serde_json::Value::Null => qexed_nbt::Tag::End,
        serde_json::Value::Bool(value) => qexed_nbt::Tag::Byte(i8::from(*value)),
        serde_json::Value::Number(value) => json_number_to_nbt(value),
        serde_json::Value::String(value) => qexed_nbt::Tag::String(Arc::from(value.as_str())),
        serde_json::Value::Array(values) => json_array_to_nbt(values),
        serde_json::Value::Object(values) => {
            let mut map = HashMap::new();
            for (key, value) in values {
                map.insert(key.clone(), json_to_nbt(value));
            }
            qexed_nbt::Tag::Compound(Arc::new(map))
        }
    }
}

fn json_number_to_nbt(value: &serde_json::Number) -> qexed_nbt::Tag {
    if let Some(value) = value.as_i64() {
        if (i32::MIN as i64..=i32::MAX as i64).contains(&value) {
            qexed_nbt::Tag::Int(value as i32)
        } else {
            qexed_nbt::Tag::Long(value)
        }
    } else if let Some(value) = value.as_u64() {
        if value <= i32::MAX as u64 {
            qexed_nbt::Tag::Int(value as i32)
        } else if value <= i64::MAX as u64 {
            qexed_nbt::Tag::Long(value as i64)
        } else {
            qexed_nbt::Tag::String(Arc::from(value.to_string()))
        }
    } else if let Some(value) = value.as_f64() {
        qexed_nbt::Tag::Float(value as f32)
    } else {
        qexed_nbt::Tag::String(Arc::from(value.to_string()))
    }
}

fn json_array_to_nbt(values: &[serde_json::Value]) -> qexed_nbt::Tag {
    if values.is_empty() {
        return qexed_nbt::Tag::List(
            qexed_nbt::ListHeader {
                tag_id: qexed_nbt::tag_id::END,
                length: 0,
            },
            Arc::from([]),
        );
    }

    let items = values.iter().map(json_to_nbt).collect::<Vec<_>>();
    let tag_id = items[0].tag_id();
    if items.iter().all(|item| item.tag_id() == tag_id) {
        qexed_nbt::Tag::List(
            qexed_nbt::ListHeader {
                tag_id,
                length: items.len() as i32,
            },
            Arc::from(items),
        )
    } else {
        let wrapped = items
            .into_iter()
            .map(|item| {
                let mut map = HashMap::new();
                map.insert("value".to_string(), item);
                qexed_nbt::Tag::Compound(Arc::new(map))
            })
            .collect::<Vec<_>>();
        qexed_nbt::Tag::List(
            qexed_nbt::ListHeader {
                tag_id: qexed_nbt::tag_id::COMPOUND,
                length: wrapped.len() as i32,
            },
            Arc::from(wrapped),
        )
    }
}

/// 数据包 → 已编码字节（packet id + payload），与 v4 `packet_bytes` 语义一致。
pub(crate) fn packet_bytes<T: Packet>(packet: T) -> Result<Bytes, EntitiesError> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}
