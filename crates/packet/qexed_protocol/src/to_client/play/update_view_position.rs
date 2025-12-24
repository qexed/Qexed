use qexed_packet::net_types::VarInt;
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x57)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateViewPosition {
    pub chunk_x:VarInt,
    pub chunk_z:VarInt,
}
