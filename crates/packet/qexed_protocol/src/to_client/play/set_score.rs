use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;

use crate::to_client::play::set_objective::NumberFormat;
#[qexed_packet_macros::packet(id = 0x67)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetScore {
    pub entity_name: String,
    pub scoreboard_name: String,
    pub value: VarInt,
    pub display_text: Option<qexed_nbt::Tag>,
    pub number_format: Option<NumberFormat>,
}
