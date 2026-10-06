use qexed_packet::{PacketCodec, net_types::*};

pub const ANCHOR_FEET: i32 = 0;
pub const ANCHOR_EYES: i32 = 1;

#[qexed_packet_macros::packet(id = 0x47)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerLookAt {
    // TODO: net.minecraft.commands.arguments.EntityAnchorArgument$Anchor 枚举 -> VarInt（0=FEET, 1=EYES）
    pub from_anchor: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub at_entity: bool,
    pub entity: VarInt,
    // TODO: 同上，仅 at_entity=true 时读写
    pub to_anchor: VarInt,
}
