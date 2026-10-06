use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundJigsawGeneratePacket` (play, to_server, id 0x1B)。
#[qexed_packet_macros::packet(id = 0x1B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct JigsawGenerate {
    pub pos: Position,
    pub levels: VarInt,
    pub keep_jigsaws: bool,
}
