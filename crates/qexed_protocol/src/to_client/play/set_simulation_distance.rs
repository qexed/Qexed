use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x71)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetSimulationDistance {
    pub simulation_distance: VarInt,
}
