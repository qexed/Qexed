use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSetTestBlockPacket` (play, to_server, id 0x3D)。
#[qexed_packet_macros::packet(id = 0x3D)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetTestBlock {
    pub position: Position,
    // TODO: TestBlockMode 枚举（START=0, LOG=1, FAIL=2, ACCEPT=3）→ VarInt。
    pub mode: VarInt,
    pub message: String,
}
