use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x45)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerCombatKill {
    pub player_id: VarInt,
    pub message: crate::types::TextComponent,
}
