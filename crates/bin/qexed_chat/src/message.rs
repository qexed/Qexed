use bytes::Bytes;
use qexed_command::message::CommandData;
use qexed_task::message::unreturn_message::UnReturnMessage;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum ManagerMessage {
    NewPlayerConnect(
        uuid::Uuid,
        bool, // 是否成功
        Option<NewPlayerConnectError>, // 报错
        Option<UnboundedSender<UnReturnMessage<TaskMessage>>>,// 任务api
    ), // Qexed Login阶段完成后进入配置阶段，配置阶段开始前会传递进服阶段前的所有数据包进行处理
    Command(CommandData),// 指令事件
    CommandMe(CommandData),// 指令事件
    CommandSay(CommandData),// 指令事件
    BroadCastEvent(uuid::Uuid,qexed_protocol::to_client::play::system_chat::SystemChat),// 广播聊天数据包
    PlayerClose(uuid::Uuid),  // 游戏连接关闭
    ConnectClose(uuid::Uuid), // 连接关闭
}
#[derive(Debug)]
pub enum TaskMessage {
    Start(
        String,
        Option<UnboundedSender<Bytes>>, // 数据包发送器
    ), // 传递数据包收发器
    ChatEvent(qexed_protocol::to_server::play::chat_message::ChatMessage),// 数据包分割器传递聊天数据包
    SendMessage(qexed_protocol::to_client::play::system_chat::SystemChat),// 广播事件数据包
    Close,// 连接关闭
}

#[derive(Error, Debug, Clone)]
pub enum NewPlayerConnectError {
    #[error("玩家未离开服务器")]
    PlayerNotAway,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
}

