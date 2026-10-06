use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x43)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerCombatEnter {}
