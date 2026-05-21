use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x12)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ShowDialog {
    pub dialog: qexed_packet::net_types::AnyNbt,
}
