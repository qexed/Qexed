use std::net::{SocketAddr};
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
}
