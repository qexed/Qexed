use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x06)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatCommand {
    pub command:String,
}