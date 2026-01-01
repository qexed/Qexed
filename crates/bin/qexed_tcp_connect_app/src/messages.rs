use std::net::{SocketAddr};

use bytes::Bytes;
use qexed_packet::Packet;
use qexed_task::message::{return_message::ReturnMessage, steam_message::SteamMessage};
#[derive(Debug)]
pub enum ManagerCommand {
    Start,
    NewConnection(tokio::net::TcpStream, SocketAddr),
    NewConnectionFinish,
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
    ListenClose(bool),// True:读关闭,False:写关闭
}

#[derive(Debug)]
pub enum WriteCommand<T: Packet> {
    Start,
    Packet(T),
    RawPacket(Bytes),
    Close,
}
#[derive(Debug)]
pub enum ReadCommand<T: Packet> {
    Start,
    Packet(ReturnMessage<T>),
    RawPacket(ReturnMessage<Vec<u8>>),
    RawPacketSteam(SteamMessage<Vec<u8>>),
    Close,
}