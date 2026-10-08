use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x20)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MovePlayerRot {
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
}
