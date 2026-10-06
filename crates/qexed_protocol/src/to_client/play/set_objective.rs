use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x6c)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetObjective {
    pub objective_name: String,
    pub method: i8,
    // TODO: method=1(remove) 时 Java 不写以下三个字段；扁平宏模板无法表达条件字段，需手工 codec
    pub display_name: TextComponent,
    // TODO: net.minecraft.world.scores.criteria.ObjectiveCriteria$RenderType -> VarInt 占位（0=integer 1=hearts）
    pub render_type: VarInt,
    pub number_format: Option<NumberFormat>,
}
