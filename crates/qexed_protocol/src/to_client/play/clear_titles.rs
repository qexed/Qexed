use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0xe)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClearTitles {
    pub reset_times: bool,
}
