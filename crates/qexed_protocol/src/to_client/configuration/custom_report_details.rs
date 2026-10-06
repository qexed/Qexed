use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x10)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomReportDetails {
    pub details: qexed_packet::net_types::StringMap,
}
