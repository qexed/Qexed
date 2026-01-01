use std::net::{SocketAddr};

use bytes::Bytes;
use tokio::sync::oneshot;
#[derive(Debug)]
pub enum ManagerCommand {
    Start,
    NewConnection(tokio::net::TcpStream, SocketAddr),
    NewConnectionFinish,
    GetStatusPackageBytes(Option<Bytes>),
    TaskClose(SocketAddr),
}
#[derive(Debug)]
pub enum ListenCommand {
    Start,
    NewConnection(tokio::net::TcpStream, SocketAddr),
    ClearRatelimit(std::net::IpAddr),
    Close,
}

#[derive(Debug)]
pub enum LogicCommand {
    Start,
    HaProxy,
    ConnectionInit,
    Handshaking,
    Status(qexed_protocol::to_server::handshaking::set_protocol::SetProtocol),
    BungeeCordProxyHandshaking(qexed_protocol::to_server::handshaking::set_protocol::SetProtocol),
    Login(qexed_protocol::to_server::handshaking::set_protocol::SetProtocol),
    ListenClose(bool),// True:读关闭,False:写关闭
    Close,// 全局关闭
}

#[derive(Debug)]
pub enum WriteCommand {
    Start,
    RawPacket(Bytes),
    Close,
}
#[derive(Debug)]
pub enum ReadCommand{
    Start,
    RawPacket(oneshot::Sender<Vec<u8>>),
    // RawPacketSteam(SteamMessage<Vec<u8>>),
    Close,
}