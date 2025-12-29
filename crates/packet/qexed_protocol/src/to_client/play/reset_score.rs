use qexed_packet::{PacketCodec};
#[qexed_packet_macros::packet(id = 0x48)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResetScore {
    pub entity_name:String,
    pub objective_name:Option<String>,
}