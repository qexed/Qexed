use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;

#[qexed_packet_macros::packet(id = 0x63)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetObjective {
    pub name:String,
    pub action:i8,
    pub display_text:qexed_nbt::Tag,
    pub r#type:VarInt,
    pub number_format:Option<NumberFormat>,
}
#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum NumberFormat {
    Blank,
    Styled(qexed_nbt::Tag),
    Fuxed(qexed_nbt::Tag),
    Unknown,
}
