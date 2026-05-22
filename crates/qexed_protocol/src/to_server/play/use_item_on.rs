use qexed_packet::{
    PacketCodec,
    net_types::{Position, VarInt},
};

#[qexed_packet_macros::packet(id = 0x42)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UseItemOn {
    pub hand: VarInt,
    pub block_hit: BlockHitResult,
    pub sequence: VarInt,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockHitResult {
    pub position: Position,
    pub face: VarInt,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_z: f32,
    pub inside_block: bool,
    pub world_border_hit: bool,
}
