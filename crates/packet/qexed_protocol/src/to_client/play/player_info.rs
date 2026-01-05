use qexed_packet::{Packet, PacketCodec, net_types::VarInt};

use crate::to_client::play;
#[derive(Debug, Default, PartialEq, Clone)]

pub struct PlayerInfo {
    pub action:u8,
    // 值含义(自己位操作判断):
    // add_player
    // initialize_chat
    // update_game_mode
    // update_listed
    // update_latency
    // update_display_name
    // update_hat
    // update_list_order
    pub players:Vec<Players>
    
}
#[derive(Debug, Default, PartialEq, Clone)]

pub struct Players{
    pub uuid:uuid::Uuid,
    pub data:Vec<PlayerActions>,
}
impl Packet for PlayerInfo {
    const ID: u32= 0x3f;
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.action.serialize(w)?;
        let players_len=VarInt(self.players.len() as i32);
        players_len.serialize(w)?;
        for i in &self.players{
            i.uuid.serialize(w)?;
            for v in &i.data{
                v.serialize(w)?;
            }
        };
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.action.deserialize(r)?;
        let mut players_len: VarInt=Default::default();
        players_len.deserialize(r)?;
        let action = get_bits_with_masks(self.action);
        for i in 0..players_len.0{
            let mut player:Players=Default::default();
            player.uuid.deserialize(r)?;
            if action[0]{
                let mut value: String=Default::default();
                value.deserialize(r)?;
                let mut value2: Vec<crate::to_client::login::success::Properties>=Default::default();
                value2.deserialize(r)?;
                player.data.push(PlayerActions::AddPlayer(value,value2));
            }
            if action[1]{
                let mut value:Option<InitializeChat>=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::InitializeChat(value));
            }
            if action[2]{
                let mut value:VarInt=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::UpdateGameMode(value));
            }
            if action[3]{
                let mut value:bool=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::UpdateListed(value));
            }
            if action[4]{
                let mut value:VarInt=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::UpdateLatency(value));
            }
            if action[5]{
                let mut value:Option<qexed_nbt::Tag>=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::UpdateDisplayName(value));
            }
            if action[6]{
                let mut value:VarInt=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::UpdateListPriority(value));
            }
            if action[7]{
                let mut value:bool=Default::default();
                value.deserialize(r)?;
                player.data.push(PlayerActions::UpdateHat(value));
            }
            self.players.push(player);
        };
        Ok(())
    }
    
    
}
#[derive(Debug,Clone,PartialEq)]

pub enum PlayerActions {
    AddPlayer(String,Vec<crate::to_client::login::success::Properties>),
    InitializeChat(Option<InitializeChat>),
    UpdateGameMode(VarInt),
    UpdateListed(bool),
    UpdateLatency(VarInt),
    UpdateDisplayName(Option<qexed_nbt::Tag>),
    UpdateListPriority(VarInt),
    UpdateHat(bool),
    Unknown
}
impl Default for PlayerActions {
    fn default() -> Self {
        PlayerActions::Unknown
    }
}
impl qexed_packet::PacketCodec for PlayerActions {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        match self {
            PlayerActions::AddPlayer(name,success) => {
                name.serialize(w)?;
                success.serialize(w)?;
            },
            PlayerActions::InitializeChat(initialize_chat) => {
                initialize_chat.serialize(w)?;
            },
            PlayerActions::UpdateGameMode(var_int) => {
                var_int.serialize(w)?;
            },
            PlayerActions::UpdateListed(b) => {
                b.serialize(w)?;
            },
            PlayerActions::UpdateLatency(var_int) => {
                var_int.serialize(w)?;
            },
            PlayerActions::UpdateDisplayName(tag) => {
                tag.serialize(w)?;
            },
            PlayerActions::UpdateListPriority(var_int) => {
                var_int.serialize(w)?;
            },
            PlayerActions::UpdateHat(b) => {
                b.serialize(w)?;
            },
            PlayerActions::Unknown => {
            },
        }
        return Ok(());
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        match self {
            PlayerActions::AddPlayer(name,success) => {
                name.deserialize(r)?;
                success.deserialize(r)?;
            },
            PlayerActions::InitializeChat(initialize_chat) => {
                initialize_chat.deserialize(r)?;
            },
            PlayerActions::UpdateGameMode(var_int) => {
                var_int.deserialize(r)?;
            },
            PlayerActions::UpdateListed(b) => {
                b.deserialize(r)?;
            },
            PlayerActions::UpdateLatency(var_int) => {
                var_int.deserialize(r)?;
            },
            PlayerActions::UpdateDisplayName(tag) => {
                tag.deserialize(r)?;
            },
            PlayerActions::UpdateListPriority(var_int) => {
                var_int.deserialize(r)?;
            },
            PlayerActions::UpdateHat(b) => {
                b.deserialize(r)?;
            },
            PlayerActions::Unknown => {
            },
        }
        return Ok(());
    }
}
#[derive(Debug, Default, PartialEq, Clone)]
#[qexed_packet_macros::substruct]
pub struct InitializeChat{
    chat_session_id:uuid::Uuid,
    public_key_expiry_time:i64,
    encoded_public_key:Vec<u8>,
    public_key_signature:Vec<u8>,
}
fn get_bits_with_masks(value: u8) -> [bool; 8] {
    [
        (value & 0b0000_0001) != 0, // 位 0 (最低位)
        (value & 0b0000_0010) != 0, // 位 1
        (value & 0b0000_0100) != 0, // 位 2
        (value & 0b0000_1000) != 0, // 位 3
        (value & 0b0001_0000) != 0, // 位 4
        (value & 0b0010_0000) != 0, // 位 5
        (value & 0b0100_0000) != 0, // 位 6
        (value & 0b1000_0000) != 0, // 位 7 (最高位)
    ]
}