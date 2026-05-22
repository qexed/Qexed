use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x0a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatSessionUpdate {
    pub chat_session: crate::types::ChatSessionData,
}
