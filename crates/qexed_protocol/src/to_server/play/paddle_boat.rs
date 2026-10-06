use qexed_packet::{PacketCodec};

/// `ServerboundPaddleBoatPacket` (play, to_server, id 0x23)。
#[qexed_packet_macros::packet(id = 0x23)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PaddleBoat {
    pub left: bool,
    pub right: bool,
}
