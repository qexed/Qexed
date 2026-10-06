use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundPickItemFromEntityPacket` (play, to_server, id 0x25)。
#[qexed_packet_macros::packet(id = 0x25)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PickItemFromEntity {
    pub id: VarInt,
    pub include_data: bool,
}
