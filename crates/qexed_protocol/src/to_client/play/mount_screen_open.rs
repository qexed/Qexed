use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x2a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MountScreenOpen {
    pub container_id: VarInt,
    pub inventory_columns: VarInt,
    pub entity_id: i32,
}
