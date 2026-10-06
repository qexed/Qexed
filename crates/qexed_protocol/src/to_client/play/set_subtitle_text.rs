use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x71)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetSubtitleText {
    pub text: TextComponent,
}
