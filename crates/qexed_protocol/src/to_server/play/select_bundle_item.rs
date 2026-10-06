use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSelectBundleItemPacket` (play, to_server, id 0x03).
#[qexed_packet_macros::packet(id = 0x03)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SelectBundleItem {
    pub slot_id: VarInt,
    pub selected_item_index: VarInt,
}
