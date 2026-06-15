use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x08)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomClickAction {
    pub id: String,
    pub payload:
        qexed_packet::net_types::LengthPrefixed<qexed_packet::net_types::OptionalNbt, 65536>,
}
