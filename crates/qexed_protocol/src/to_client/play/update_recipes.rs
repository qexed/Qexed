use crate::types::{IDSet, SlotDisplay};
use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x87)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateRecipes {
    pub item_sets: Vec<RecipePropertySetEntry>,
    pub stonecutter_recipes: Vec<StonecutterRecipeEntry>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipePropertySetEntry {
    // TODO: net.minecraft.resources.ResourceKey<RecipePropertySet> -> String 占位（注册表键）
    pub key: String,
    pub items: Vec<VarInt>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StonecutterRecipeEntry {
    pub input: IDSet,
    pub option_display: SlotDisplay,
}
