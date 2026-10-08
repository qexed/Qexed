use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x74)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SoundEntity {
    pub sound_event: VarInt,
    pub sound_source: VarInt,
    pub entity_id: VarInt,
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
}

#[qexed_packet_macros::packet(id = 0x75)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Sound {
    pub sound_event: VarInt,
    pub sound_source: VarInt,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
}

impl Sound {
    pub fn at_block(sound_event: i32, sound_source: i32, x: f64, y: f64, z: f64) -> Self {
        Self {
            sound_event: VarInt(sound_event),
            sound_source: VarInt(sound_source),
            x: pack_sound_coord(x),
            y: pack_sound_coord(y),
            z: pack_sound_coord(z),
            volume: 1.0,
            pitch: 1.0,
            seed: 0,
        }
    }
}

fn pack_sound_coord(value: f64) -> i32 {
    (value * 8.0).round() as i32
}
