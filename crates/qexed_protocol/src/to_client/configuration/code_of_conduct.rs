use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x13)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CodeOfConduct {
    pub code_of_conduct: String,
}
