use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;
#[qexed_packet_macros::packet(id = 0x4b)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookRemove {
    pub recipe_ids: Vec<VarInt>,
}
