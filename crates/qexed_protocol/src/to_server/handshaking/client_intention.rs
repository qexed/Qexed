use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClientIntention {
    pub protocol_version: VarInt,
    pub host_name: String,
    pub port: u16,
    // TODO: net.minecraft.network.protocol.handshake.ClientIntent 枚举占位为 VarInt（0=STATUS, 1=LOGIN, 2=TRANSFER）
    pub intention: VarInt,
}
