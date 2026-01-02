use std::net::IpAddr;

use qexed_command::message::CommandData;
use qexed_task::message::MessageSender;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum ManagerMessage {
    Command(CommandData),
    GetTcpConnectStart(Option<MessageSender<TcpConnectStartMessage>>),
}

// 子任务

// TCP 连接开始
#[derive(Debug)]
pub enum TcpConnectStartMessage{
    BanIpConnectSpeedToHigh(IpAddr),
    CheckIpConnectSppedIsFinish(IpAddr),
    CheckIpConnectSpeedToHigh(IpAddr,oneshot::Sender<bool>),
}