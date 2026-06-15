use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x35)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCarriedItem {
    pub slot: i16,
}
