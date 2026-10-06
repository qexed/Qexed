use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x81)]
#[derive(Debug, PartialEq, Clone)]
pub struct TickingState {
    pub tick_rate: f32,
    pub frozen: bool,
}

impl Default for TickingState {
    fn default() -> Self {
        Self {
            tick_rate: 20.0,
            frozen: false,
        }
    }
}
