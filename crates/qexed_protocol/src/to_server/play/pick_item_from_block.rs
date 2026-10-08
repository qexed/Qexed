use qexed_packet::{PacketCodec, net_types::Position};

#[qexed_packet_macros::packet(id = 0x24)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PickItemFromBlock {
    pub position: Position,
    pub include_data: bool,
}
