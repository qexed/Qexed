use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x41)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerAbilities {
    pub flags: u8,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl PlayerAbilities {
    pub const INVULNERABLE: u8 = 0x01;
    pub const FLYING: u8 = 0x02;
    pub const CAN_FLY: u8 = 0x04;
    pub const INSTABUILD: u8 = 0x08;
}
