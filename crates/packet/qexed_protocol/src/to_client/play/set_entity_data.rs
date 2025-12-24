use qexed_packet::{PacketCodec, net_types::VarInt};

use crate::types::EntityMetadata;
#[qexed_packet_macros::packet(id = 0x5C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEntityData {
    pub entity_id:VarInt,
    pub metadata: EntityMetadata, // 是否显示在动作栏而非聊天框
}