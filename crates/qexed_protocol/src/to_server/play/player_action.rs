use qexed_packet::{
    PacketCodec,
    net_types::{Position, VarInt},
};
#[qexed_packet_macros::packet(id = 0x29)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerAction {
    pub status: VarInt,
    pub location: Position,
    pub face: i8,
    pub sequence: VarInt,
}
