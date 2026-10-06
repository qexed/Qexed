use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x17)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomChatCompletions {
    pub action: VarInt,
    pub entries: Vec<String>,
}

impl CustomChatCompletions {
    pub const ADD: i32 = 0;
    pub const REMOVE: i32 = 1;
    pub const SET: i32 = 2;
}
