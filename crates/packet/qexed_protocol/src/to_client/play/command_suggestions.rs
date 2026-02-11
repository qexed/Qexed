use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x0F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CommandSuggestions {
    pub id:VarInt,
    pub start:VarInt,
    pub length:VarInt,
    pub matches:Vec<Matches>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Matches {
    pub r#match:String,
    pub tooltip:Option<qexed_nbt::Tag>,
}