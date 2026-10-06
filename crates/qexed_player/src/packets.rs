use bytes::{Bytes, BytesMut};
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client::play::{
    add_entity::AddEntity,
    entity_position_sync::EntityPositionSync,
    player_info_remove::PlayerInfoRemove,
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    remove_entities::RemoveEntities,
    rotate_head::RotateHead,
    set_equipment::SetEquipment,
};
use qexed_protocol::types::EntityPosition;

use crate::{OnlinePlayer, PlayerEvent};
use crate::error::Result;

impl PlayerEvent {
    pub fn packets(
        &self,
        player_entity_type: i32,
        viewer_dimension: &str,
    ) -> Result<Vec<bytes::Bytes>> {
        match self {
            Self::Joined(player) => {
                if player.dimension == viewer_dimension {
                    spawn_player_packets(player, player_entity_type)
                } else {
                    Ok(vec![player_info_packet(player)?])
                }
            }
            Self::Left {
                profile_id,
                entity_id,
                username: _,
                dimension,
            } => {
                let mut packets = Vec::new();
                if dimension == viewer_dimension {
                    packets.push(packet_bytes(RemoveEntities::one(*entity_id))?);
                }
                packets.push(packet_bytes(PlayerInfoRemove::one(*profile_id))?);
                Ok(packets)
            }
            Self::Moved {
                profile_id: _,
                entity_id,
                dimension,
                position,
            } => {
                if dimension != viewer_dimension {
                    return Ok(Vec::new());
                }
                Ok(vec![
                    packet_bytes(EntityPositionSync {
        id: qexed_packet::net_types::VarInt(*entity_id),
        position: qexed_protocol::to_client::play::entity_position_sync::PositionPath::Linear {
            x: position.x, y: position.y, z: position.z,
        },
        y_rot: position.yaw,
        x_rot: position.pitch,
        on_ground: position.on_ground,
    })?,
                    packet_bytes(RotateHead::from_degrees(*entity_id, position.yaw))?,
                ])
            }
            Self::Teleport {
                profile_id: _,
                dimension: _,
                position: _,
            } => Ok(Vec::new()),
            Self::Damage {
                profile_id: _,
                amount: _,
                kind: _,
                source_entity_id: _,
                source_position: _,
                knockback: _,
            } => Ok(Vec::new()),
            Self::PotionEffect {
                profile_id: _,
                effect: _,
                amplifier: _,
                duration_ticks: _,
                source_entity_id: _,
                source_position: _,
                knockback: _,
            } => Ok(Vec::new()),
            Self::GameModeChanged {
                profile_id,
                username: _,
                game_mode,
            } => Ok(vec![game_mode_update_packet(*profile_id, *game_mode)?]),
            Self::GiveItem {
                profile_id: _,
                item: _,
                item_name: _,
            } => Ok(Vec::new()),
            Self::ProjectileHitPlayer(_) => Ok(Vec::new()),
            Self::Animation {
                profile_id: _,
                entity_id,
                dimension,
                action_id,
            } => {
                if dimension != viewer_dimension {
                    return Ok(Vec::new());
                }
                Ok(vec![packet_bytes(
                    qexed_protocol::to_client::play::animate::Animate {
                        entity_id: qexed_packet::net_types::VarInt(*entity_id),
                        action_id: *action_id,
                    },
                )?])
            }
            Self::DimensionChanged {
                profile_id: _,
                entity_id,
                old_dimension,
                player,
            } => {
                if old_dimension == viewer_dimension {
                    return Ok(vec![packet_bytes(RemoveEntities::one(*entity_id))?]);
                }
                if player.dimension == viewer_dimension {
                    return spawn_player_packets(player, player_entity_type);
                }
                Ok(Vec::new())
            }
            Self::SetEquipmentChanged {
                profile_id: _,
                entity_id,
                dimension,
                slots,
            } => {
                if dimension != viewer_dimension {
                    return Ok(Vec::new());
                }
                Ok(vec![packet_bytes(SetEquipment {
                    entity: qexed_packet::net_types::VarInt(*entity_id),
                    slots: slots.clone(),
                })?])
            }
            Self::BlockChanged {
                profile_id: _,
                dimension,
                position,
                block_state,
                light_update,
            } => {
                if dimension != viewer_dimension {
                    return Ok(Vec::new());
                }
                let mut packets = vec![packet_bytes(
                    qexed_protocol::to_client::play::block_update::BlockUpdate {
                        location: position.clone(),
                        block_state: qexed_packet::net_types::VarInt(*block_state),
                    },
                )?];
                if let Some(light_update) = light_update {
                    packets.push(light_update.clone());
                }
                Ok(packets)
            }
            Self::BlockChanges {
                profile_id: _,
                dimension,
                changes,
            } => {
                if dimension != viewer_dimension {
                    return Ok(Vec::new());
                }
                changes
                    .iter()
                    .map(|change| {
                        packet_bytes(qexed_protocol::to_client::play::block_update::BlockUpdate {
                            location: change.position.clone(),
                            block_state: qexed_packet::net_types::VarInt(change.block_state),
                        })
                    })
                    .collect()
            }
            Self::ClientboundPackets { packets } => Ok(packets.clone()),
        }
    }
}

pub fn spawn_player_packets(
    player: &OnlinePlayer,
    player_entity_type: i32,
) -> Result<Vec<Bytes>> {
    Ok(vec![
        player_info_packet(player)?,
        packet_bytes(
            spawn_add_entity(
                player.entity_id,
                player.profile.uuid,
                player_entity_type,
                player.position,
            ),
        )?,
        packet_bytes(RotateHead::from_degrees(player.entity_id, player.position.yaw))?,
        packet_bytes(SetEquipment {
                    entity: qexed_packet::net_types::VarInt(player.entity_id),
            slots: player.equipment.clone(),
        })?,
    ])
}

fn player_info_packet(player: &OnlinePlayer) -> Result<Bytes> {
    packet_bytes(PlayerInfoUpdate {
        actions: PlayerInfoActions::player_initializing(),
        entries: vec![PlayerInfoEntry::from_profile(
            &player.profile,
            player.game_mode,
        )],
    })
}

fn game_mode_update_packet(profile_id: uuid::Uuid, game_mode: i32) -> Result<Bytes> {
    packet_bytes(PlayerInfoUpdate {
        actions: PlayerInfoActions(PlayerInfoActions::UPDATE_GAME_MODE),
        entries: vec![PlayerInfoEntry {
            profile_id,
            game_mode: qexed_packet::net_types::VarInt(game_mode),
            ..PlayerInfoEntry::default()
        }],
    })
}

pub fn packet_bytes<T: Packet>(packet: T) -> Result<Bytes> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}

/// v4 AddEntity::player 的 v6 等价构造（LpVec3 movement 版）。
fn spawn_add_entity(
    entity_id: i32,
    profile_uuid: uuid::Uuid,
    entity_type: i32,
    position: EntityPosition,
) -> AddEntity {
    use qexed_protocol::to_client::play::add_entity::LpVec3;
    AddEntity {
        id: qexed_packet::net_types::VarInt(entity_id),
        uuid: profile_uuid,
        entity_type: qexed_packet::net_types::VarInt(entity_type),
        x: position.x,
        y: position.y,
        z: position.z,
        movement: LpVec3::default(),
        x_rot: pack_degrees(position.pitch),
        y_rot: pack_degrees(position.yaw),
        y_head_rot: pack_degrees(position.yaw),
        data: qexed_packet::net_types::VarInt(0),
    }
}

fn pack_degrees(degrees: f32) -> i8 {
    ((degrees * 256.0 / 360.0).floor() as i32 & 0xff) as i8
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_spawn_packets_do_not_send_removed_skin_parts_metadata() {
        let player = OnlinePlayer {
            profile: qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::from_u128(1),
                username: "Player".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            game_mode: 0,
            position: qexed_protocol::types::EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
            displayed_skin_parts: 0x7f,
        };

        let packets = spawn_player_packets(&player, 155).unwrap();
        let set_entity_data_id =
            qexed_protocol::to_client::play::set_entity_data::SetEntityData::ID as u8;

        assert!(
            packets
                .iter()
                .all(|packet| packet.first() != Some(&set_entity_data_id))
        );
    }

    #[test]
    fn player_info_uses_player_game_mode() {
        let player = OnlinePlayer {
            profile: qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::from_u128(2),
                username: "SurvivalPlayer".to_string(),
                properties: Vec::new(),
            },
            entity_id: 2,
            game_mode: 0,
            position: qexed_protocol::types::EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
            displayed_skin_parts: 0x7f,
        };

        let packet = player_info_packet(&player).unwrap();
        let mut payload = packet.slice(1..);
        let mut decoded = PlayerInfoUpdate::default();
        decoded
            .deserialize(&mut qexed_packet::PacketReader::new(&mut payload))
            .unwrap();

        assert_eq!(decoded.entries[0].game_mode.0, 0);
    }

    #[test]
    fn animation_packets_respect_viewer_dimension() {
        let event = PlayerEvent::Animation {
            profile_id: uuid::Uuid::from_u128(3),
            entity_id: 42,
            dimension: "minecraft:overworld".to_string(),
            action_id: 0,
        };

        let same_dimension_packets = event.packets(155, "minecraft:overworld").unwrap();
        assert_eq!(same_dimension_packets.len(), 1);
        assert_eq!(
            same_dimension_packets[0].first(),
            Some(&(qexed_protocol::to_client::play::animate::Animate::ID as u8))
        );

        let other_dimension_packets = event.packets(155, "minecraft:the_nether").unwrap();
        assert!(other_dimension_packets.is_empty());
    }
}
