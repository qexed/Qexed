use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSetCommandMinecartPacket` (play, to_server, id 0x38)。
#[qexed_packet_macros::packet(id = 0x38)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCommandMinecart {
    pub entity: VarInt,
    pub command: String,
    pub track_output: bool,
}
