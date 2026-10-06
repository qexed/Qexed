use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x67)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEquipment {
    pub entity: VarInt,
    // TODO: Java 编码无长度前缀：逐项 slot 字节（除最后一项均置 0x80 继续位）+ Slot；
    // 扁平宏的 Vec 会先写长度，slot 也非字节编码，需手工 codec 修正
    pub slots: Vec<EquipmentEntry>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EquipmentEntry {
    // TODO: net.minecraft.world.entity.EquipmentSlot -> VarInt 占位（0=mainhand..7=saddle）
    pub slot: VarInt,
    pub item: Slot,
}
