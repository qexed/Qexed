use bytes::Bytes;
use qexed_task::message::{ unreturn_message::UnReturnMessage};
use thiserror::Error;
use tokio::sync::{mpsc::{ UnboundedSender}, oneshot};

#[derive(Debug,Clone)]
pub enum ManagerCommand{
    NewPlayerConnect(uuid::Uuid,bool,Option<NewPlayerConnectError>,Option<UnboundedSender<UnReturnMessage<TaskCommand>>>,Option<UnboundedSender<Bytes>>),
    PlayerClose(uuid::Uuid),// 游戏连接关闭
}
#[derive(Debug)]
pub enum TaskCommand{
    Start, // 传递数据包收发器
    Pause,// 暂停
    Stop,// 关闭
    UpdatePart(Part),
    Await(oneshot::Sender<bool>),// 或者保证在下一场执行Ping之前拦截
    Pong(i32),// Ping的返回结果
    Close,// 关闭服务
}
#[derive(Debug,Clone)]
pub enum Part {
    Configuration,
    Play,
}

#[derive(Error, Debug,Clone)]
pub enum NewPlayerConnectError {
    #[error("玩家未离开服务器")]
    PlayerNotAway,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
}