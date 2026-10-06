use qexed_packet::{PacketCodec, net_types::{OptionalVarInt, Position, VarInt}};

#[qexed_packet_macros::packet(id = 0x31)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Login {
    // 注意：playerId 用 ByteBufCodecs.INT（普通 i32），不是 VarInt
    pub player_id: i32,
    pub hardcore: bool,
    // TODO: Set<ResourceKey<Level>> — Identifier 字符串集合
    pub levels: Vec<String>,
    pub max_players: VarInt,
    pub chunk_radius: VarInt,
    pub simulation_distance: VarInt,
    pub reduced_debug_info: bool,
    pub show_death_screen: bool,
    pub do_limited_crafting: bool,
    pub common_player_spawn_info: CommonPlayerSpawnInfo,
    pub online_mode: bool,
    pub enforces_secure_chat: bool,
}

/// net.minecraft.network.protocol.game.CommonPlayerSpawnInfo。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CommonPlayerSpawnInfo {
    // TODO: Holder<DimensionType> — 注册表 id，用 VarInt 占位
    pub dimension_type: VarInt,
    // TODO: ResourceKey<Level> — Identifier 字符串
    pub dimension: String,
    pub seed: i64,
    // TODO: GameType — idMapper VarInt（0=survival 1=creative 2=adventure 3=spectator）
    pub game_mode: VarInt,
    // OPTIONAL_VAR_INT：0 表示无上一模式
    pub previous_game_mode: OptionalVarInt,
    pub is_debug: bool,
    pub is_flat: bool,
    pub last_death_location: Option<GlobalPos>,
    pub portal_cooldown: VarInt,
    pub sea_level: VarInt,
}

impl PacketCodec for CommonPlayerSpawnInfo {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.dimension_type.serialize(w)?;
        self.dimension.serialize(w)?;
        self.seed.serialize(w)?;
        self.game_mode.serialize(w)?;
        self.previous_game_mode.serialize(w)?;
        self.is_debug.serialize(w)?;
        self.is_flat.serialize(w)?;
        self.last_death_location.serialize(w)?;
        self.portal_cooldown.serialize(w)?;
        self.sea_level.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.dimension_type.deserialize(r)?;
        self.dimension.deserialize(r)?;
        self.seed.deserialize(r)?;
        self.game_mode.deserialize(r)?;
        self.previous_game_mode.deserialize(r)?;
        self.is_debug.deserialize(r)?;
        self.is_flat.deserialize(r)?;
        self.last_death_location.deserialize(r)?;
        self.portal_cooldown.deserialize(r)?;
        self.sea_level.deserialize(r)
    }
}

/// net.minecraft.core.GlobalPos：维度 Identifier + BlockPos。
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct GlobalPos {
    // TODO: ResourceKey<Level> — Identifier 字符串
    pub dimension: String,
    pub pos: Position,
}

impl PacketCodec for GlobalPos {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.dimension.serialize(w)?;
        self.pos.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.dimension.deserialize(r)?;
        self.pos.deserialize(r)
    }
}
