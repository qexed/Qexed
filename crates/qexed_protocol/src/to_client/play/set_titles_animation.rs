use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x75)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetTitlesAnimation {
    pub fade_in: i32,
    pub stay: i32,
    pub fade_out: i32,
}
