use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x59)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetActionBarText {
    pub text: crate::types::TextComponent,
}
