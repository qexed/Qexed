use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x21)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StatusOnly {
// TODO: 26.3 移动标志包（flags 位掩码），待展开
pub flags: u8,
}
