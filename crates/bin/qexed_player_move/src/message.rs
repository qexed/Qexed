// use bytes::Bytes;
// use qexed_command::message::CommandData;
// use qexed_task::message::unreturn_message::UnReturnMessage;
// use thiserror::Error;
// use tokio::sync::mpsc::UnboundedSender;

// #[derive(Debug)]
// pub enum ManagerMessage {
//     NewPlayerConnect(
//         uuid::Uuid,
//         bool, // 是否成功
//         Option<NewPlayerConnectError>, // 报错
//         Option<UnboundedSender<UnReturnMessage<TaskMessage>>>,// 任务api
//     ), // Qexed Login阶段完成后进入配置阶段，配置阶段开始前会传递进服阶段前的所有数据包进行处理
//     Command(CommandData),// 指令事件
//     PlayerClose(uuid::Uuid),  // 游戏连接关闭
//     ConnectClose(uuid::Uuid), // 连接关闭
// }
// #[derive(Debug)]
// pub enum TaskMessage {
//     Start(
//         String,
//         Option<UnboundedSender<Bytes>>, // 数据包发送器
//     ), // 传递数据包收发器
//     SendTitleMessage(qexed_protocol::to_client::play::set_title_text::SetTitleText),// 广播事件数据包
//     Close,// 连接关闭
// }

// #[derive(Debug)]
// pub enum SystemEvent{
//     PlayerJoin,
//     PlayerLevel,
// }
// #[derive(Error, Debug, Clone)]
// pub enum NewPlayerConnectError {
//     #[error("玩家未离开服务器")]
//     PlayerNotAway,
//     #[error("Invalid VarLong - too many bytes")]
//     InvalidVarLong,
// }

