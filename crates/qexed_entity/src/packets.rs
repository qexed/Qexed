use anyhow::Result;
use bytes::{Bytes, BytesMut};
use qexed_packet::{
    Packet, PacketCodec,
    net_types::{GameProfile, ProfileProperty, VarInt},
};
use qexed_protocol::{
    to_client::play::{
        add_entity::{AddEntity, EntityPositionSync, PlayerInfoRemove, RemoveEntities, RotateHead},
        player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
        set_entity_data::SetEntityData,
        take_item_entity::TakeItemEntity,
    },
    types::{EntityMetadata, EntityMetadataEnum, EntityMetadataSub},
};
use std::{collections::HashMap, sync::Arc};

use crate::{DroppedItemEntity, ManagedEntity, ManagedEntityKind};

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

impl ManagedEntity {
    pub fn spawn_packets(&self) -> Result<Vec<Bytes>> {
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

        let add_entity = if player_npc {
            AddEntity::player(
                self.entity_id,
                self.uuid,
                self.entity_type_id,
                self.position,
            )
        } else {
            AddEntity::new(
                self.entity_id,
                self.uuid,
                self.entity_type_id,
                self.position,
                self.data,
            )
        };
        packets.push(packet_bytes(add_entity)?);
        packets.push(packet_bytes(RotateHead::new(
            self.entity_id,
            self.position.yaw,
        ))?);

        let metadata = match self.kind {
            ManagedEntityKind::Hologram => Some(hologram_metadata(self.display_name())),
            ManagedEntityKind::Npc | ManagedEntityKind::Entity if player_npc => {
                Some(player_entity_metadata(self.display_name()))
            }
            ManagedEntityKind::Npc => self.display_name().map(named_entity_metadata),
            ManagedEntityKind::Entity => None,
        };
        if let Some(metadata) = metadata {
            packets.push(packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata,
            })?);
        }

        Ok(packets)
    }

    pub fn remove_packets(&self) -> Result<Vec<Bytes>> {
        let mut packets = vec![packet_bytes(RemoveEntities::one(self.entity_id))?];
        if self.kind == ManagedEntityKind::Npc && self.entity_type == "minecraft:player" {
            packets.push(packet_bytes(PlayerInfoRemove::one(self.uuid))?);
        }
        Ok(packets)
    }

    pub fn position_packets(&self) -> Result<Vec<Bytes>> {
        self.position_packets_with_velocity(0.0, 0.0, 0.0)
    }

    pub fn position_packets_with_velocity(
        &self,
        velocity_x: f64,
        velocity_y: f64,
        velocity_z: f64,
    ) -> Result<Vec<Bytes>> {
        Ok(vec![
            packet_bytes(EntityPositionSync::from_position_with_velocity(
                self.entity_id,
                self.position,
                velocity_x,
                velocity_y,
                velocity_z,
            ))?,
            packet_bytes(RotateHead::new(self.entity_id, self.position.yaw))?,
        ])
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
    pub fn spawn_packets(&self, entity_type_id: i32) -> Result<Vec<Bytes>> {
        Ok(vec![
            packet_bytes(AddEntity::new(
                self.entity_id,
                self.uuid,
                entity_type_id,
                self.position,
                0,
            ))?,
            packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata: item_entity_metadata(self.item.clone()),
            })?,
        ])
    }

    pub fn pickup_packets(&self, collector_entity_id: i32) -> Result<Vec<Bytes>> {
        let amount = self.item.item_count.0.max(1);
        Ok(vec![
            packet_bytes(TakeItemEntity {
                item_id: VarInt(self.entity_id),
                player_id: VarInt(collector_entity_id),
                amount: VarInt(amount),
            })?,
            packet_bytes(RemoveEntities::one(self.entity_id))?,
        ])
    }

    pub fn metadata_packets(&self) -> Result<Vec<Bytes>> {
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
    display_name: Option<qexed_protocol::types::TextComponent>,
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

fn item_entity_metadata(item: qexed_protocol::types::Slot) -> EntityMetadata {
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

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(Arc::new(map))
}

fn text_component_or_json(text: &str) -> qexed_protocol::types::TextComponent {
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

fn json_text_component(text: &str) -> Option<qexed_protocol::types::TextComponent> {
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

fn packet_bytes<T: Packet>(packet: T) -> Result<Bytes> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}

#[cfg(test)]
mod tests {
    use qexed_packet::{Packet, PacketCodec};
    use qexed_protocol::to_client::play::add_entity::{EntityPosition, EntityPositionSync};

    use super::*;

    #[test]
    fn ordinary_entity_spawn_does_not_include_display_name_metadata() {
        let entity = ManagedEntity {
            key: "zombie".to_string(),
            entity_id: 7,
            uuid: uuid::Uuid::new_v4(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id: 1,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 1.0,
                y: 64.0,
                z: 2.0,
                yaw: 90.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Zombie".to_string(),
            display_name: "Cluster Zombie".to_string(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: String::new(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        };

        let packets = entity.spawn_packets().unwrap();
        let packet_ids = packets
            .into_iter()
            .map(|mut packet| {
                let mut reader = qexed_packet::PacketReader::new(&mut packet);
                let mut packet_id = VarInt::default();
                packet_id.deserialize(&mut reader).unwrap();
                packet_id.0
            })
            .collect::<Vec<_>>();

        assert_eq!(
            packet_ids,
            vec![
                qexed_protocol::to_client::play::add_entity::AddEntity::ID,
                qexed_protocol::to_client::play::add_entity::RotateHead::ID,
            ]
        );
    }

    #[test]
    fn position_packets_include_velocity() {
        let entity = ManagedEntity {
            key: "zombie".to_string(),
            entity_id: 7,
            uuid: uuid::Uuid::new_v4(),
            kind: ManagedEntityKind::Entity,
            entity_type: "minecraft:zombie".to_string(),
            entity_type_id: 1,
            dimension: "minecraft:overworld".to_string(),
            position: EntityPosition {
                x: 1.0,
                y: 64.0,
                z: 2.0,
                yaw: 90.0,
                pitch: 0.0,
                on_ground: true,
            },
            name: "Zombie".to_string(),
            display_name: String::new(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: String::new(),
            ai_params: Default::default(),
            auto_jump: false,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: "interact".to_string(),
            off_hand_event: "interact_off_hand".to_string(),
            attack_event: "attack".to_string(),
        };

        let packets = entity
            .position_packets_with_velocity(0.12, 0.0, -0.04)
            .unwrap();
        let mut payload = packets[0].clone();
        let mut reader = qexed_packet::PacketReader::new(&mut payload);
        let mut packet_id = VarInt::default();
        packet_id.deserialize(&mut reader).unwrap();
        let mut sync = EntityPositionSync::default();
        sync.deserialize(&mut reader).unwrap();

        assert_eq!(packet_id.0, EntityPositionSync::ID);
        assert_eq!(sync.velocity_x, 0.12);
        assert_eq!(sync.velocity_y, 0.0);
        assert_eq!(sync.velocity_z, -0.04);
    }
}
