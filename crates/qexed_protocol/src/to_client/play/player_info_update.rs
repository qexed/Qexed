use bytes::BufMut as _;
use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{GameProfile, ProfileProperty, VarInt},
};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerInfoUpdate {
    pub actions: PlayerInfoActions,
    pub entries: Vec<PlayerInfoEntry>,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct PlayerInfoActions(pub u8);

impl PlayerInfoActions {
    pub const ADD_PLAYER: u8 = 1 << 0;
    pub const INITIALIZE_CHAT: u8 = 1 << 1;
    pub const UPDATE_GAME_MODE: u8 = 1 << 2;
    pub const UPDATE_LISTED: u8 = 1 << 3;
    pub const UPDATE_LATENCY: u8 = 1 << 4;
    pub const UPDATE_DISPLAY_NAME: u8 = 1 << 5;
    pub const UPDATE_LIST_ORDER: u8 = 1 << 6;
    pub const UPDATE_HAT: u8 = 1 << 7;

    pub fn player_initializing() -> Self {
        Self(
            Self::ADD_PLAYER
                | Self::INITIALIZE_CHAT
                | Self::UPDATE_GAME_MODE
                | Self::UPDATE_LISTED
                | Self::UPDATE_LATENCY
                | Self::UPDATE_DISPLAY_NAME
                | Self::UPDATE_LIST_ORDER
                | Self::UPDATE_HAT,
        )
    }

    fn contains(self, action: u8) -> bool {
        self.0 & action != 0
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerInfoEntry {
    pub profile_id: uuid::Uuid,
    pub profile_name: String,
    pub properties: Vec<ProfileProperty>,
    pub chat_session: Option<crate::types::ChatSessionData>,
    pub game_mode: VarInt,
    pub listed: bool,
    pub latency: VarInt,
    pub display_name: Option<crate::types::TextComponent>,
    pub list_order: VarInt,
    pub show_hat: bool,
}

impl PlayerInfoEntry {
    pub fn from_profile(profile: &GameProfile, game_mode: i32) -> Self {
        Self {
            profile_id: profile.uuid,
            profile_name: profile.username.clone(),
            properties: profile.properties.clone(),
            chat_session: None,
            game_mode: VarInt(game_mode),
            listed: true,
            latency: VarInt(0),
            display_name: None,
            list_order: VarInt(0),
            show_hat: true,
        }
    }
}

impl Packet for PlayerInfoUpdate {
    const ID: i32 = 0x47;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.actions.serialize(w)?;
        VarInt(self.entries.len() as i32).serialize(w)?;
        for entry in &self.entries {
            entry.profile_id.serialize(w)?;
            write_action_payloads(self.actions, entry, w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.actions.deserialize(r)?;
        let mut count = VarInt::default();
        count.deserialize(r)?;
        if count.0 < 0 {
            return Err(qexed_packet::PacketError::msg(format!("negative player info entry count: {}", count.0)));
        }

        self.entries.clear();
        self.entries.reserve(count.0 as usize);
        for _ in 0..count.0 {
            let mut entry = PlayerInfoEntry::default();
            entry.profile_id.deserialize(r)?;
            read_action_payloads(self.actions, &mut entry, r)?;
            self.entries.push(entry);
        }
        Ok(())
    }
}

impl PacketCodec for PlayerInfoActions {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        w.buf.put_u8(self.0);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut value = 0_u8;
        value.deserialize(r)?;
        self.0 = value;
        Ok(())
    }
}

fn write_action_payloads(
    actions: PlayerInfoActions,
    entry: &PlayerInfoEntry,
    w: &mut PacketWriter,
) -> qexed_packet::Result<()> {
    if actions.contains(PlayerInfoActions::ADD_PLAYER) {
        entry.profile_name.serialize(w)?;
        entry.properties.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::INITIALIZE_CHAT) {
        entry.chat_session.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_GAME_MODE) {
        entry.game_mode.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_LISTED) {
        entry.listed.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_LATENCY) {
        entry.latency.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_DISPLAY_NAME) {
        entry.display_name.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_LIST_ORDER) {
        entry.list_order.serialize(w)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_HAT) {
        entry.show_hat.serialize(w)?;
    }
    Ok(())
}

fn read_action_payloads(
    actions: PlayerInfoActions,
    entry: &mut PlayerInfoEntry,
    r: &mut PacketReader,
) -> qexed_packet::Result<()> {
    if actions.contains(PlayerInfoActions::ADD_PLAYER) {
        entry.profile_name.deserialize(r)?;
        entry.properties.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::INITIALIZE_CHAT) {
        entry.chat_session.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_GAME_MODE) {
        entry.game_mode.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_LISTED) {
        entry.listed.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_LATENCY) {
        entry.latency.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_DISPLAY_NAME) {
        entry.display_name.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_LIST_ORDER) {
        entry.list_order.deserialize(r)?;
    }
    if actions.contains(PlayerInfoActions::UPDATE_HAT) {
        entry.show_hat.deserialize(r)?;
    }
    Ok(())
}
