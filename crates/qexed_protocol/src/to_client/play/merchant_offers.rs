use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

/// net.minecraft.world.item.trading.ItemCost：Holder(VarInt) + count(VarInt) + 组件谓词。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ItemCost {
    // TODO: net.minecraft.core.Holder<net.minecraft.world.item.Item> 注册表 id，用 VarInt 占位
    pub item: VarInt,
    pub count: VarInt,
    // TODO: net.minecraft.core.component.DataComponentExactPredicate 暂用组件列表占位
    pub components: Vec<ComponentsToAdd>,
}

/// net.minecraft.world.item.trading.MerchantOffer。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MerchantOffer {
    pub base_cost_a: ItemCost,
    /// result 为 ItemStack（OptionaL_STREAM_CODEC 之外的完整槽位格式）。
    pub result: Slot,
    pub cost_b: Option<ItemCost>,
    pub out_of_stock: bool,
    pub uses: i32,
    pub max_uses: i32,
    pub villager_xp: i32,
    pub special_price_diff: i32,
    pub price_multiplier: f32,
    pub demand: i32,
}

#[qexed_packet_macros::packet(id = 0x35)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MerchantOffers {
    pub container_id: VarInt,
    pub offers: Vec<MerchantOffer>,
    pub villager_level: VarInt,
    pub villager_xp: VarInt,
    pub show_progress: bool,
    pub can_restock: bool,
}
