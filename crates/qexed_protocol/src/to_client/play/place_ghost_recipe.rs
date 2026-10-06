use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x40)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlaceGhostRecipe {
    pub container_id: VarInt,
    // TODO: net.minecraft.world.item.crafting.display.RecipeDisplay 为注册表分发类型，
    // 复用 types::RecipeDisplay（VarInt id + 载荷）；id 顺序待与 26.3 注册表核对
    pub recipe_display: crate::types::RecipeDisplay,
}
