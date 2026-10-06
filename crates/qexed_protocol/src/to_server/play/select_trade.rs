use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSelectTradePacket` (play, to_server, id 0x34)。
#[qexed_packet_macros::packet(id = 0x34)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SelectTrade {
    pub item: VarInt,
}
