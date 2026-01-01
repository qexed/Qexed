use std::net::{IpAddr, SocketAddr};

use bytes::Bytes;
use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;
#[derive(Debug)]
pub enum ManagerCommand {
    Start,
    ConnClose(SocketAddr),
    GetStatusPackageBytes(Option<Bytes>),
    CheckPlayeIsInList(uuid::Uuid, bool),
    LoginCheck(uuid::Uuid, Option<IpAddr>, bool, Option<String>),
    GetLogicApi(qtunnel_server_logic::message::ManagerMessage),
    Shutdown(String),
}
#[derive(Debug)]
pub enum TaskCommand {
    Start,
    ConnClose(SocketAddr),
    Shutdown(String),
}
