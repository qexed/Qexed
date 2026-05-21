use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x26)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct KeepAlive {
    pub keep_alive_id:i64,
}
