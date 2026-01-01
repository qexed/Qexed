use bytes::Bytes;
use qexed_task::message::unreturn_message::UnReturnMessage;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum ManagerCommand {
    NewPlayerConnect(
        uuid::Uuid,
        bool,
        Option<NewPlayerConnectError>,
        Option<UnboundedSender<UnReturnMessage<TaskCommand>>>,
        Option<UnboundedSender<Bytes>>,
    ),
    PlayerClose(uuid::Uuid),
    HeartbeatStatus(uuid::Uuid, HeartbeatStatus),
}

#[derive(Debug, Clone)]
pub enum HeartbeatStatus {
    Alive(u64,HeartbeatPhase),        // 最后心跳时间戳
    Timeout(uuid::Uuid,HeartbeatPhase), // 心跳超时
    Disconnected(uuid::Uuid,HeartbeatPhase), // 连接断开
}

/// 心跳阶段枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatPhase {
    /// 配置阶段（包括登录阶段）
    Configuration,
    /// 游戏阶段
    Play,
}

impl Default for HeartbeatPhase {
    fn default() -> Self {
        Self::Configuration
    }
}

// 内部消息类型
#[derive(Debug)]
pub enum InternalMessage {
    SendHeartbeat,
    HeartbeatReceived(i64),
    StateChange(StateChange),
    PhaseChange(HeartbeatPhase),  // 修改：使用 HeartbeatPhase 枚举
    Shutdown,
}

#[derive(Debug)]
pub enum StateChange {
    Running(bool),
    Paused(bool),
}
#[derive(Debug)]
pub enum TaskCommand {
    Start,
    Pause,
    Stop,
    Heartbeat(i64),          // 心跳信号
    Part(bool),// False:配置阶段,True:游戏阶段
    // CheckTimeout,      // 检查超时
    Close,
}

#[derive(Error, Debug, Clone)]
pub enum NewPlayerConnectError {
    #[error("玩家已存在")]
    PlayerAlreadyExists,
    #[error("无效的连接通道")]
    InvalidConnection,
}