use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x86)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateAttributes {
    pub entity_id: VarInt,
    pub attributes: Vec<AttributeValue>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AttributeValue {
    pub attribute_id: VarInt,
    pub base_value: f64,
    pub modifiers: Vec<AttributeModifier>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AttributeModifier {
    pub id: String,
    pub amount: f64,
    pub operation: VarInt,
}
