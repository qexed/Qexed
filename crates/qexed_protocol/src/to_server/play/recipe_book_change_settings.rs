use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundRecipeBookChangeSettingsPacket` (play, to_server, id 0x2F)。
#[qexed_packet_macros::packet(id = 0x2F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookChangeSettings {
    // TODO: net.minecraft.world.inventory.RecipeBookType 枚举
    //  （CRAFTING=0, FURNACE=1, BLAST_FURNACE=2, SMOKER=3）→ VarInt。
    pub book_type: VarInt,
    pub is_open: bool,
    pub is_filtering: bool,
}
