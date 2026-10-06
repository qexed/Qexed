use qexed_packet::{PacketCodec, net_types::Position};

#[qexed_packet_macros::packet(id = 0x62)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetDefaultSpawnPosition {
    pub dimension: String,
    pub position: Position,
    pub yaw: f32,
    pub pitch: f32,
}
