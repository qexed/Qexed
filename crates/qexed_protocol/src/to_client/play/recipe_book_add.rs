use qexed_packet::PacketCodec;
use qexed_packet::net_types::{OptionalVarInt, VarInt};

use crate::types::{IDSet, RecipeDisplay};
#[qexed_packet_macros::packet(id = 0x4a)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookAdd {
    pub entries: Vec<Recipes>,
    pub replace: bool,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Recipes {
    pub recipe: VarInt,
    pub display: RecipeDisplay,
    pub group: OptionalVarInt,
    pub category: VarInt,
    pub ingredients: Option<Vec<IDSet>>,
    pub flags: u8,
}
