use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x83)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TickingStep {
    pub tick_steps: VarInt,
}
