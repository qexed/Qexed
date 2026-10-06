use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x6F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetScore {
    pub owner: String,
    pub objective_name: String,
    pub score: VarInt,
    pub display: Option<crate::types::TextComponent>,
    pub number_format: Option<crate::types::NumberFormat>,
}

impl SetScore {
    pub fn new(
        owner: impl Into<String>,
        objective_name: impl Into<String>,
        score: i32,
        display: Option<crate::types::TextComponent>,
    ) -> Self {
        Self {
            owner: owner.into(),
            objective_name: objective_name.into(),
            score: VarInt(score),
            display,
            number_format: Some(crate::types::NumberFormat::Blank),
        }
    }
}
