use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::{Position, VarInt}};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Login {
    pub entity_id: i32,
    pub is_hardcore: bool,
    pub dimension_names: Vec<String>,
    pub max_player: VarInt,
    pub view_distance: VarInt,       // 视距
    pub simulation_distance: VarInt, // 模拟距离
    pub reduced_debug_info: bool,
    pub enable_respawn_screen: bool,
    pub do_limited_crafting: bool,
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
    pub enforces_secure_chat: bool,
}

impl Packet for Login {
    const ID: u32 = 0x2b;
    fn serialize(&self, w: &mut PacketWriter) -> Result<(), anyhow::Error> {
        w.serialize(&self.entity_id)?;
        w.serialize(&self.is_hardcore)?;
        w.serialize(&self.dimension_names)?;
        w.serialize(&self.max_player)?;
        w.serialize(&self.view_distance)?;
        w.serialize(&self.simulation_distance)?;
        w.serialize(&self.reduced_debug_info)?;
        w.serialize(&self.enable_respawn_screen)?;
        w.serialize(&self.do_limited_crafting)?;
        w.serialize(&self.dimension_type)?;
        w.serialize(&self.dimension_name)?;
        w.serialize(&self.hashed_seed)?;
        w.serialize(&self.game_mode)?;
        w.serialize(&self.previous_game_mode)?;
        w.serialize(&self.is_debug)?;
        w.serialize(&self.is_flat)?;
        w.serialize(&self.has_death_location)?;
        if self.has_death_location {
            w.serialize(&self.death_dimension_name)?;
            w.serialize(&self.death_position)?;
        }
        w.serialize(&self.portal_cooldown)?;
        w.serialize(&self.sea_level)?;
        w.serialize(&self.enforces_secure_chat)?;
        Ok(())
    }
    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), anyhow::Error> {
        self.entity_id.deserialize(r)?;
        self.is_hardcore.deserialize(r)?;
        self.dimension_names.deserialize(r)?;
        self.max_player.deserialize(r)?;
        self.view_distance.deserialize(r)?;
        self.simulation_distance.deserialize(r)?;
        self.reduced_debug_info.deserialize(r)?;
        self.enable_respawn_screen.deserialize(r)?;
        self.do_limited_crafting.deserialize(r)?;
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
        }
        self.portal_cooldown.deserialize(r)?;
        self.sea_level.deserialize(r)?;
        self.enforces_secure_chat.deserialize(r)?;
        Ok(())
    }
}
