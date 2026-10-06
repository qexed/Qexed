use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x32)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LowDiskSpaceWarning {}
