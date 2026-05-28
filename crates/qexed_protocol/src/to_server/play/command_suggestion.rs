use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x0F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CommandSuggestion {
    pub id: VarInt,
    pub text: String,
}
