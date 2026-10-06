use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundPlayerCommandPacket` (play, to_server, id 0x2A)。
#[qexed_packet_macros::packet(id = 0x2A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerCommand {
    pub id: VarInt,
    // TODO: ServerboundPlayerCommandPacket.Action 枚举
    //  （STOP_SLEEPING=0, START_SPRINTING=1, STOP_SPRINTING=2, START_RIDING_JUMP=3,
    //   STOP_RIDING_JUMP=4, OPEN_INVENTORY=5, START_FALL_FLYING=6）→ VarInt。
    pub action: VarInt,
    pub data: VarInt,
}
