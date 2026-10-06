use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x5C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBorderWarningDelay {
    pub warning_delay: VarInt,
}
