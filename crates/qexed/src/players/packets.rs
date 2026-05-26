use bytes::{Bytes, BytesMut};
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client::play::{
    add_entity::{EntityPositionSync, PlayerInfoRemove, RemoveEntities, RotateHead},
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    set_equipment::SetEquipment,
};

use super::{OnlinePlayer, PlayerEvent};

impl PlayerEvent {
    pub fn packets(&self, player_entity_type: i32) -> anyhow::Result<Vec<bytes::Bytes>> {
        match self {
            Self::Joined(player) => spawn_player_packets(player, player_entity_type),
            Self::Left {
                profile_id,
                entity_id,
                username: _,
            } => Ok(vec![
                packet_bytes(RemoveEntities::one(*entity_id))?,
                packet_bytes(PlayerInfoRemove::one(*profile_id))?,
            ]),
            Self::Moved {
                profile_id: _,
                entity_id,
                position,
            } => Ok(vec![
                packet_bytes(EntityPositionSync::from_position(*entity_id, *position))?,
                packet_bytes(RotateHead::new(*entity_id, position.yaw))?,
            ]),
            Self::EquipmentChanged {
                profile_id: _,
                entity_id,
                slots,
            } => Ok(vec![packet_bytes(SetEquipment {
                entity_id: qexed_packet::net_types::VarInt(*entity_id),
                slots: slots.clone(),
            })?]),
            Self::BlockChanged {
                profile_id: _,
                position,
                block_state,
                light_update,
            } => {
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
        packet_bytes(PlayerInfoUpdate {
            actions: PlayerInfoActions::player_initializing(),
            entries: vec![PlayerInfoEntry::from_profile(&player.profile, 1)],
        })?,
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

pub(crate) fn packet_bytes<T: Packet>(packet: T) -> anyhow::Result<Bytes> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
    packet.serialize(&mut writer)?;
    Ok(buf.freeze())
}
