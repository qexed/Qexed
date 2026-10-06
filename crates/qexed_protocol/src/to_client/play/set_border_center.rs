use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x59)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBorderCenter {
    pub new_center_x: f64,
    pub new_center_z: f64,
}
