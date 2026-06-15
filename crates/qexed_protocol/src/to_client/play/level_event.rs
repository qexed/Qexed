use qexed_packet::{PacketCodec, net_types::Position};

#[qexed_packet_macros::packet(id = 0x2E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LevelEvent {
    pub event_id: i32,
    pub position: Position,
    pub data: i32,
    pub global_event: bool,
}
