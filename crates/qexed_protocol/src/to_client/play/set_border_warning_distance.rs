use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x5e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBorderWarningDistance {
    pub warning_blocks: VarInt,
}
