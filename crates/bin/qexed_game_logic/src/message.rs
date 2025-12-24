use bytes::Bytes;
use qexed_player::Player;
use qexed_task::message::return_message::ReturnMessage;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum ManagerMessage {
    NewPlayerConnect(
        uuid::Uuid,
        bool,
        Option<NewPlayerConnectError>,
        Option<UnboundedSender<ReturnMessage<TaskMessage>>>,
    ), // Qexed Login阶段完成后进入配置阶段，配置阶段开始前会传递进服阶段前的所有数据包进行处理
    // GetConnect(uuid::Uuid,UnboundedSender<TaskMessage>), // 获取玩家连接管道
    // PlayerPlayPart(uuid::Uuid),// 玩家进入游戏阶段
    Registry(
        Option<Vec<qexed_protocol::to_client::configuration::registry_data::RegistryData>>,
        Option<qexed_protocol::to_client::configuration::tags::Tags>,
    ),
    GetPlayerPing(Option<qexed_ping::message::ManagerCommand>),
    GetPlayerHeartbeat(Option<qexed_heartbeat::message::ManagerCommand>),
    GetPlayerPacketSplit(Option<qexed_packet_split::message::ManagerMessage>),
    GetPlayerChat(Option<qexed_chat::message::ManagerMessage>),
    GetTitle(Option<qexed_title::message::ManagerMessage>),
    GetCommand(Option<qexed_command::message::ManagerCommand>),
    GetPlayerListApi(Option<UnboundedSender<ReturnMessage<qexed_player_list::Message>>>),
    PlayerClose(uuid::Uuid),  // 游戏连接关闭
    ConnectClose(uuid::Uuid), // 连接关闭
}
#[derive(Debug)]
pub enum TaskMessage {
    Start(
        Player,
        Option<UnboundedReceiver<Vec<u8>>>,
        Option<UnboundedSender<Bytes>>,
    ), // 传递数据包收发器
    Configuration(bool),
    Play,  // 游戏阶段
    Close, // 连接关闭
}

#[derive(Error, Debug, Clone)]
pub enum NewPlayerConnectError {
    #[error("玩家未离开服务器")]
    PlayerNotAway,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
}
