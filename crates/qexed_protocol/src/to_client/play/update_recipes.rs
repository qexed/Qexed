use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};
use qexed_packet::error::PacketError;

use crate::types::{IDSet, SlotDisplay};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateRecipes {
    pub item_sets: Vec<RecipePropertySetEntry>,
    pub stonecutter_recipes: Vec<StonecutterRecipeEntry>,
}

impl Packet for UpdateRecipes {
    const ID: i32 = 0x85;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.item_sets.serialize(w)?;
        self.stonecutter_recipes.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        self.item_sets.deserialize(r)?;
        self.stonecutter_recipes.deserialize(r)
    }
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipePropertySetEntry {
    pub key: String,
    pub items: Vec<VarInt>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StonecutterRecipeEntry {
    pub input: IDSet,
    pub option_display: SlotDisplay,
}
