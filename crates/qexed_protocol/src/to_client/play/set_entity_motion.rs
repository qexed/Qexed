use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x65)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEntityMotion {
    pub entity_id: VarInt,
    pub delta_x: i16,
    pub delta_y: i16,
    pub delta_z: i16,
}

impl SetEntityMotion {
    pub fn from_velocity(entity_id: i32, x: f64, y: f64, z: f64) -> Self {
        Self {
            entity_id: VarInt(entity_id),
            delta_x: pack_velocity(x),
            delta_y: pack_velocity(y),
            delta_z: pack_velocity(z),
        }
    }
}

fn pack_velocity(value: f64) -> i16 {
    (value.clamp(-3.9, 3.9) * 8000.0).round() as i16
}
