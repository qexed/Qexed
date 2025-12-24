use serde::{Deserialize, Serialize};

use crate::{
    app::{
        qexed_blacklist::BlackList, qexed_chat::ChatConfig, qexed_chunk::ChunkConfig, qexed_command::CommandConfig, qexed_entity_id_allocator::EntityIdAllocator, qexed_game_logic::GameLogicConfig, qexed_heartbeat::HeartbeatConfig, qexed_packet_split::PacketSplitConfig, qexed_ping::PingConfig, qexed_player_list::PlayerList, qexed_rule::RuleConfig, qexed_status::StatusConfig, qexed_tcp_connect_app::TcpConnect, qexed_whitelist::WhiteList
    },
    tool::AppConfigTrait,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct One {
    pub version: i32,
    pub tcp_connect_app: TcpConnect,
    pub player_list: PlayerList,
    pub server_status: StatusConfig,
    pub white_list: WhiteList,
    pub black_list: BlackList,
    pub chat:ChatConfig,
    pub game_logic: GameLogicConfig,
    pub ping: PingConfig,
    pub heartbeat:HeartbeatConfig,
    pub entity_id_allocator:EntityIdAllocator,
    pub packet_split:PacketSplitConfig,
    pub command:CommandConfig,
    pub rule:RuleConfig,
    pub chunk:ChunkConfig,
    
}
impl AppConfigTrait for One {
    const PATH: &'static str = "./config/";

    const NAME: &'static str = "qexed";
}
