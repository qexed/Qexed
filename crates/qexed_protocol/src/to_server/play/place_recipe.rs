use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundPlaceRecipePacket` (play, to_server, id 0x27)。
#[qexed_packet_macros::packet(id = 0x27)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlaceRecipe {
    pub container_id: VarInt,
    // TODO: net.minecraft.world.item.crafting.display.RecipeDisplayId（配方展示索引）→ VarInt。
    pub recipe: VarInt,
    pub use_max_items: bool,
}
