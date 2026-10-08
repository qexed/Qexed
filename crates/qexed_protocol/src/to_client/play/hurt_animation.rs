use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x2A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct HurtAnimation {
    pub entity_id: VarInt,
    pub yaw: f32,
}
