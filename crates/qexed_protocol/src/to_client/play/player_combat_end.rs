use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x42)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerCombatEnd {
    pub duration: VarInt,
}
