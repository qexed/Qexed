use qexed_packet::{
    PacketCodec,
    net_types::{Position, VarInt},
};

#[qexed_packet_macros::packet(id = 0x05)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockDestruction {
    pub entity_id: VarInt,
    pub location: Position,
    pub destroy_stage: i8,
}
