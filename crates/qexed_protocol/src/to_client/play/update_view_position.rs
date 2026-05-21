use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;
#[qexed_packet_macros::packet(id = 0x5e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateViewPosition {
    pub chunk_x: VarInt,
    pub chunk_z: VarInt,
}
