use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x72)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetTime {
    pub game_time: i64,
    pub clock_updates: Vec<ClockUpdate>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClockUpdate {
    pub clock: VarInt,
    pub value: ClockNetworkState,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClockNetworkState {
    pub value: f32,
    pub rate: f32,
}
