use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClientIntention {
    pub protocol_version: VarInt,
    pub host_name: String,
    pub port: u16,
    // 线上语义（v4 实测同款）：1=STATUS, 2=LOGIN, 3=TRANSFER
    pub intention: VarInt,
}
