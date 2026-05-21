use qexed_packet::net_types::VarInt;
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x58)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateViewDistance {
    pub view_distance:VarInt,
}
