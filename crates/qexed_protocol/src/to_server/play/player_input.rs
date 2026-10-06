use qexed_packet::PacketCodec;

pub const FORWARD: u8 = 0x01;
pub const BACKWARD: u8 = 0x02;
pub const LEFT: u8 = 0x04;
pub const RIGHT: u8 = 0x08;
pub const JUMP: u8 = 0x10;
pub const SHIFT: u8 = 0x20;
pub const SPRINT: u8 = 0x40;

#[qexed_packet_macros::packet(id = 0x2b)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerInput {
    pub flags: u8,
}

impl PlayerInput {
    pub fn jump(&self) -> bool {
        self.flags & JUMP != 0
    }
}
