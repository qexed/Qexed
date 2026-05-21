use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;
#[qexed_packet_macros::packet(id = 0x5f)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateViewDistance {
    pub view_distance: VarInt,
}
