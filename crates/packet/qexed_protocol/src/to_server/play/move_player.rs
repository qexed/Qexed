use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x1D)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MovePlayerPos {
    pub x:f64,
    pub feed_y:f64, 
    pub z:f64,
    pub flags:u8,
}
#[qexed_packet_macros::packet(id = 0x1E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MovePlayerPosRot {
    pub x:f64,
    pub feed_y:f64,
    pub z:f64,
    pub yaw:f32,
    pub pitch:f32,
    pub flags:u8,
}

#[qexed_packet_macros::packet(id = 0x1f)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MovePlayerRot {
    pub yaw:f32,
    pub pitch:f32,
    pub flags:u8,
}