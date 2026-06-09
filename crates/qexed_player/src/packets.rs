use bytes::{Bytes, BytesMut};
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client::play::{
    add_entity::{EntityPositionSync, PlayerInfoRemove, RemoveEntities, RotateHead},
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    set_equipment::SetEquipment,
};

use crate::{OnlinePlayer, PlayerEvent};

impl PlayerEvent {
    pub fn packets(
        &self,
        player_entity_type: i32,
        viewer_dimension: &str,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
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
                    packet_bytes(EntityPositionSync::from_position(*entity_id, *position))?,
                    packet_bytes(RotateHead::new(*entity_id, position.yaw))?,
                ])
            }
            Self::Teleport {
                profile_id: _,
                dimension: _,
                position: _,
            } => Ok(Vec::new()),
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
            Self::EquipmentChanged {
                profile_id: _,
                entity_id,
                dimension,
                slots,
            } => {
                if dimension != viewer_dimension {
                    return Ok(Vec::new());
                }
                Ok(vec![packet_bytes(SetEquipment {
                    entity_id: qexed_packet::net_types::VarInt(*entity_id),
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
            Self::ClientboundPackets { packets } => Ok(packets.clone()),
        }
    }
}

pub fn spawn_player_packets(
    player: &OnlinePlayer,
    player_entity_type: i32,
) -> anyhow::Result<Vec<Bytes>> {
    Ok(vec![
        player_info_packet(player)?,
        packet_bytes(
            qexed_protocol::to_client::play::add_entity::AddEntity::player(
                player.entity_id,
                player.profile.uuid,
                player_entity_type,
                player.position,
            ),
        )?,
        packet_bytes(RotateHead::new(player.entity_id, player.position.yaw))?,
        packet_bytes(SetEquipment {
            entity_id: qexed_packet::net_types::VarInt(player.entity_id),
            slots: player.equipment.clone(),
        })?,
    ])
}

fn player_info_packet(player: &OnlinePlayer) -> anyhow::Result<Bytes> {
    packet_bytes(PlayerInfoUpdate {
        actions: PlayerInfoActions::player_initializing(),
        entries: vec![PlayerInfoEntry::from_profile(
            &player.profile,
            player.game_mode,
        )],
    })
}

pub fn packet_bytes<T: Packet>(packet: T) -> anyhow::Result<Bytes> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
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
            position: qexed_protocol::to_client::play::add_entity::EntityPosition {
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
            position: qexed_protocol::to_client::play::add_entity::EntityPosition {
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
}
