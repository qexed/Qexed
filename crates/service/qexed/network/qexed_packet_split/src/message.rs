use bytes::Bytes;
use qexed_task::message::{return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum ManagerMessage {
    NewPlayerConnect(
        uuid::Uuid,
        bool, // 是否成功
        Option<NewPlayerConnectError>, // 报错
        Option<UnboundedSender<ReturnMessage<TaskMessage>>>,// 任务api
    ), // Qexed Login阶段完成后进入配置阶段，配置阶段开始前会传递进服阶段前的所有数据包进行处理
    // GetConnect(uuid::Uuid,UnboundedSender<TaskMessage>), // 获取玩家连接管道
    // PlayerPlayPart(uuid::Uuid),// 玩家进入游戏阶段
    PlayerClose(uuid::Uuid),  // 游戏连接关闭
    ConnectClose(uuid::Uuid), // 连接关闭
}
#[derive(Debug)]
pub enum TaskMessage {
    Start(
        qexed_player::Player,
        Option<UnboundedReceiver<Vec<u8>>>, // 数据包接收器
        Option<UnboundedSender<Bytes>>, // 数据包发送器
        // Option<UnboundedSender<UnReturnMessage<qexed_ping::message::TaskCommand>>>,// Ping服务:由上层服务 qexed_game_logic 提供
        Option<UnboundedSender<UnReturnMessage<qexed_heartbeat::message::TaskCommand>>>,// 心跳服务
        Option<UnboundedSender<UnReturnMessage<qexed_chat::message::TaskMessage>>>,// 聊天服务
        Option<UnboundedSender<UnReturnMessage<qexed_command::message::TaskCommand>>>
    ), // 传递数据包收发器
    Run, // 暂时没实现数据包分割器
    Close,                           // 连接关闭
}

#[derive(Error, Debug, Clone)]
pub enum NewPlayerConnectError {
    #[error("玩家未离开服务器")]
    PlayerNotAway,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
}
