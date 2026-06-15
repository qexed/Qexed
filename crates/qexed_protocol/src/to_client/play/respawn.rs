use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{Position, VarInt},
};

pub const KEEP_ATTRIBUTE_MODIFIERS: i8 = 1;
pub const KEEP_ENTITY_DATA: i8 = 2;
pub const KEEP_NO_DATA: i8 = 0;
pub const KEEP_ALL_DATA: i8 = KEEP_ATTRIBUTE_MODIFIERS | KEEP_ENTITY_DATA;

#[derive(Debug, PartialEq, Clone)]
pub struct Respawn {
    pub dimension_type: VarInt,
    pub dimension_name: String,
    pub hashed_seed: i64,
    pub game_mode: u8,
    pub previous_game_mode: i8,
    pub is_debug: bool,
    pub is_flat: bool,
    pub has_death_location: bool,
    pub death_dimension_name: Option<String>,
    pub death_position: Option<Position>,
    pub portal_cooldown: VarInt,
    pub sea_level: VarInt,
    pub data_to_keep: i8,
}

impl Default for Respawn {
    fn default() -> Self {
        Self {
            dimension_type: VarInt(1),
            dimension_name: "minecraft:overworld".to_string(),
            hashed_seed: 0,
            game_mode: 0,
            previous_game_mode: -1,
            is_debug: false,
            is_flat: false,
            has_death_location: false,
            death_dimension_name: None,
            death_position: None,
            portal_cooldown: VarInt(0),
            sea_level: VarInt(63),
            data_to_keep: KEEP_ALL_DATA,
        }
    }
}

impl Packet for Respawn {
    const ID: i32 = 0x52;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.dimension_type.serialize(w)?;
        self.dimension_name.serialize(w)?;
        self.hashed_seed.serialize(w)?;
        self.game_mode.serialize(w)?;
        self.previous_game_mode.serialize(w)?;
        self.is_debug.serialize(w)?;
        self.is_flat.serialize(w)?;
        self.has_death_location.serialize(w)?;
        if self.has_death_location {
            self.death_dimension_name.serialize(w)?;
            self.death_position.serialize(w)?;
        }
        self.portal_cooldown.serialize(w)?;
        self.sea_level.serialize(w)?;
        self.data_to_keep.serialize(w)?;
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.dimension_type.deserialize(r)?;
        self.dimension_name.deserialize(r)?;
        self.hashed_seed.deserialize(r)?;
        self.game_mode.deserialize(r)?;
        self.previous_game_mode.deserialize(r)?;
        self.is_debug.deserialize(r)?;
        self.is_flat.deserialize(r)?;
        self.has_death_location.deserialize(r)?;
        if self.has_death_location {
            self.death_dimension_name.deserialize(r)?;
            self.death_position.deserialize(r)?;
        } else {
            self.death_dimension_name = None;
            self.death_position = None;
        }
        self.portal_cooldown.deserialize(r)?;
        self.sea_level.deserialize(r)?;
        self.data_to_keep.deserialize(r)?;
        Ok(())
    }
}
