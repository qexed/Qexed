use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundRecipeBookSeenRecipePacket` (play, to_server, id 0x30)。
#[qexed_packet_macros::packet(id = 0x30)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookSeenRecipe {
    // TODO: net.minecraft.world.item.crafting.display.RecipeDisplayId（配方展示索引）→ VarInt。
    pub recipe: VarInt,
}
