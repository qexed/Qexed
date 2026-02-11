use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x03)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Compress {
    pub threshold:qexed_packet::net_types::VarInt,
}
