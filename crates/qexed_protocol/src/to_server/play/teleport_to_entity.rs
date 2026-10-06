use qexed_packet::{PacketCodec};

/// `ServerboundTeleportToEntityPacket` (play, to_server, id 0x40)。
#[qexed_packet_macros::packet(id = 0x40)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TeleportToEntity {
    pub uuid: uuid::Uuid,
}
