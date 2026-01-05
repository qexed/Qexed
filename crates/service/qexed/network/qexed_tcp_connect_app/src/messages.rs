use std::net::{IpAddr, SocketAddr};

use bytes::Bytes;
use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::oneshot;
#[derive(Debug)]
pub enum ManagerCommand {
    Start,
    NewConnection(tokio::net::TcpStream, SocketAddr),
    UpdatePlayerUUID(SocketAddr,uuid::Uuid),
    NewConnectionFinish,
    GetStatusPackageBytes(Option<Bytes>),
    CheckIsInBlockList(uuid::Uuid,Option<String>),
    CheckIsInWhiteList(uuid::Uuid,Option<String>),
    CheckPlayeIsOnline(uuid::Uuid, bool),
    KickPlayer(uuid::Uuid,String),
    GetLogicApi(qexed_game_logic::message::ManagerMessage),
    TaskClose(SocketAddr,Option<uuid::Uuid>),
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
    Finish,
    ListenClose(bool),// True:读关闭,False:写关闭
    Close(String,oneshot::Sender<()>),// 全局关闭
}

#[derive(Debug)]
pub enum WriteCommand {
    Start,
    RawPacket(Bytes),
    SetCompression(bool,oneshot::Sender<()>),
    Close,
}
#[derive(Debug)]
pub enum ReadCommand{
    Start,
    RawPacket(oneshot::Sender<Vec<u8>>),
    SetCompression(bool,oneshot::Sender<()>),
    // RawPacketSteam(SteamMessage<Vec<u8>>),
    Close,
}