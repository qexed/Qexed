use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x20)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Rot {
pub yaw: i8,
pub pitch: i8,
pub on_ground: bool,
}
