use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x11)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerButtonClick {
    pub window_id: VarInt,
    pub button_id: VarInt,
}
