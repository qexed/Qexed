use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundEntityTagQueryPacket` (play, to_server, id 0x19)。
#[qexed_packet_macros::packet(id = 0x19)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityTagQuery {
    pub transaction_id: VarInt,
    pub entity_id: VarInt,
}
