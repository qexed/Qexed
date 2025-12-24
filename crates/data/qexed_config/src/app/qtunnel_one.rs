use serde::{Deserialize, Serialize};

use crate::{
    app::{
        qexed_blacklist::BlackList, qexed_command::CommandConfig, qexed_heartbeat::HeartbeatConfig, qexed_player_list::PlayerList, qexed_status::StatusConfig, qexed_whitelist::WhiteList, qtunnel_server_logic::ServerLogicConfig, qtunnel_tcp_connect_app::TcpConnect
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
    pub command: CommandConfig,
    pub server_logic: ServerLogicConfig,
    pub heartbeat:HeartbeatConfig,
}
impl AppConfigTrait for One {
    const PATH: &'static str = "./config/";

    const NAME: &'static str = "qtunnel";
}
