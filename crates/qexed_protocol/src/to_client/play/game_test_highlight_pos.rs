use qexed_packet::{PacketCodec, net_types::Position};

#[qexed_packet_macros::packet(id = 0x29)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct GameTestHighlightPos {
    pub absolute_pos: Position,
    pub relative_pos: Position,
}
