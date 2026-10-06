use qexed_packet::{PacketCodec};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x72)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetSubtitleText {
    pub text: TextComponent,
}
