use qexed_packet::net_types::VarInt;
use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x44)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookRemove {
    pub recipe_ids:Vec<VarInt>,
}
