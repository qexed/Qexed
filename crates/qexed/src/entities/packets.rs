use anyhow::Result;
use bytes::Bytes;
use qexed_packet::net_types::{GameProfile, VarInt};
use qexed_protocol::{
    to_client::play::{
        add_entity::{AddEntity, EntityPositionSync, PlayerInfoRemove, RemoveEntities, RotateHead},
        player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
        set_entity_data::SetEntityData,
        take_item_entity::TakeItemEntity,
    },
    types::{EntityMetadata, EntityMetadataEnum, EntityMetadataSub},
};

use super::{DroppedItemEntity, ManagedEntity, ManagedEntityKind};

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
        if self.kind == ManagedEntityKind::Npc {
            packets.push(crate::players::packet_bytes(PlayerInfoUpdate {
                actions: PlayerInfoActions::player_initializing(),
                entries: vec![npc_player_info_entry(&self.profile(), 0)],
            })?);
        }

        packets.push(crate::players::packet_bytes(AddEntity::new(
            self.entity_id,
            self.uuid,
            self.entity_type_id,
            self.position,
            self.data,
        ))?);
        packets.push(crate::players::packet_bytes(RotateHead::new(
            self.entity_id,
            self.position.yaw,
        ))?);

        let metadata = match self.kind {
            ManagedEntityKind::Hologram => Some(hologram_metadata(self.display_name())),
            ManagedEntityKind::Npc | ManagedEntityKind::Entity => {
                self.display_name().map(named_entity_metadata)
            }
        };
        if let Some(metadata) = metadata {
            packets.push(crate::players::packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata,
            })?);
        }

        Ok(packets)
    }

    pub fn remove_packets(&self) -> Result<Vec<Bytes>> {
        let mut packets = vec![crate::players::packet_bytes(RemoveEntities::one(
            self.entity_id,
        ))?];
        if self.kind == ManagedEntityKind::Npc {
            packets.push(crate::players::packet_bytes(PlayerInfoRemove::one(
                self.uuid,
            ))?);
        }
        Ok(packets)
    }

    pub fn position_packets(&self) -> Result<Vec<Bytes>> {
        Ok(vec![
            crate::players::packet_bytes(EntityPositionSync::from_position(
                self.entity_id,
                self.position,
            ))?,
            crate::players::packet_bytes(RotateHead::new(self.entity_id, self.position.yaw))?,
        ])
    }

    fn profile(&self) -> GameProfile {
        GameProfile {
            uuid: self.uuid,
            username: npc_profile_name(self.profile_name()),
            properties: Vec::new(),
        }
    }

    fn display_name(&self) -> Option<&str> {
        let name = self.name.trim();
        if !name.is_empty() {
            return Some(name);
        }
        if self.kind == ManagedEntityKind::Npc {
            Some(&self.key)
        } else {
            None
        }
    }

    fn profile_name(&self) -> &str {
        self.display_name().unwrap_or(&self.key)
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
                data: Some(EntityMetadataEnum::TextComponent(text_component(
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

impl DroppedItemEntity {
    pub fn spawn_packets(&self, entity_type_id: i32) -> Result<Vec<Bytes>> {
        Ok(vec![
            crate::players::packet_bytes(AddEntity::new(
                self.entity_id,
                self.uuid,
                entity_type_id,
                self.position,
                0,
            ))?,
            crate::players::packet_bytes(SetEntityData {
                entity_id: VarInt(self.entity_id),
                metadata: item_entity_metadata(self.item.clone()),
            })?,
        ])
    }

    pub fn pickup_packets(&self, collector_entity_id: i32) -> Result<Vec<Bytes>> {
        let amount = self.item.item_count.0.max(1);
        Ok(vec![
            crate::players::packet_bytes(TakeItemEntity {
                item_id: VarInt(self.entity_id),
                player_id: VarInt(collector_entity_id),
                amount: VarInt(amount),
            })?,
            crate::players::packet_bytes(RemoveEntities::one(self.entity_id))?,
        ])
    }
}

fn npc_player_info_entry(profile: &GameProfile, game_mode: i32) -> PlayerInfoEntry {
    let mut entry = PlayerInfoEntry::from_profile(profile, game_mode);
    entry.listed = false;
    entry
}

pub(super) fn npc_profile_name(name: &str) -> String {
    let mut username = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .take(16)
        .collect::<String>();

    while username.starts_with('_') {
        username.remove(0);
    }
    while username.ends_with('_') {
        username.pop();
    }
    if username.is_empty() {
        username.push_str("NPC");
    }
    while username.len() < 3 {
        username.push('_');
    }
    username
}

fn named_entity_metadata(name: &str) -> EntityMetadata {
    EntityMetadata {
        data: vec![
            EntityMetadataSub {
                index: 2,
                data: Some(EntityMetadataEnum::OptionTextComponent(Some(
                    text_component(name),
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
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}
