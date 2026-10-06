use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x4F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResetScore {
    pub owner: String,
    pub objective_name: Option<String>,
}
