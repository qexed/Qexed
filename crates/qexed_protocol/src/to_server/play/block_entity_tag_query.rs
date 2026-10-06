use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundBlockEntityTagQueryPacket` (play, to_server, id 0x02).
#[qexed_packet_macros::packet(id = 0x02)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockEntityTagQuery {
    pub transaction_id: VarInt,
    pub pos: Position,
}
