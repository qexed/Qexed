use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x13)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ShowDialog {
    pub dialog: qexed_packet::net_types::AnyNbt,
}
