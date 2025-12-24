use qexed_packet::net_types::VarInt;
use qexed_packet::PacketCodec;

use crate::types::{IDSet, RecipeDisplay};
#[qexed_packet_macros::packet(id = 0x43)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookAdd {
    pub entries:Vec<Recipes>,
    pub replace:bool,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Recipes {
    pub recipe:VarInt,
    pub display:RecipeDisplay,
    pub group:VarInt,
    pub category:VarInt,
    pub ingredients:Option<Vec<IDSet>>,
    pub flags:u8,
}