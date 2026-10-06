use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSetJigsawBlockPacket` (play, to_server, id 0x3B)。
#[qexed_packet_macros::packet(id = 0x3B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetJigsawBlock {
    pub pos: Position,
    pub name: String,
    pub target: String,
    pub pool: String,
    pub final_state: String,
    // TODO: JigsawBlockEntity.JointType 枚举（ROLLABLE="rollable", ALIGNED="aligned"），
    //  线上写的是 getSerializedName 字符串，此处用 String 占位。
    pub joint: String,
    pub selection_priority: VarInt,
    pub placement_priority: VarInt,
}
