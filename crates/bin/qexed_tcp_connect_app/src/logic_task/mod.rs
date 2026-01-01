use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};

use crate::messages::{LogicCommand, ManagerCommand};

pub mod hyproxy;
pub mod read_task;
pub mod write_task;
#[derive(Debug)]
pub struct LogicTask {
    stream: Option<tokio::net::TcpStream>,
    addr: std::net::SocketAddr,
    online_mode: bool,
    network_compression_threshold: usize,
    proxy: bool,
    proxy_protocol: qexed_config::app::qexed_tcp_connect_app::ForwardingMode,
    proxy_token: String,
    haproxy_protocol: bool,
}
impl LogicTask {
    pub fn new(
        stream: tokio::net::TcpStream,
        addr: std::net::SocketAddr,
        online_mode: bool,
        network_compression_threshold: usize,
        proxy: bool,
        proxy_protocol: qexed_config::app::qexed_tcp_connect_app::ForwardingMode,
        proxy_token: String,
        haproxy_protocol: bool,
    ) -> Self {
        Self {
            stream: Some(stream),
            addr,
            online_mode,
            network_compression_threshold,
            proxy,
            proxy_protocol,
            proxy_token,
            haproxy_protocol,
        }
    }
}

#[async_trait::async_trait]
impl TaskEvent<LogicCommand, ReturnMessage<ManagerCommand>> for LogicTask {
    async fn event(
        &mut self,
        api: &MessageSender<LogicCommand>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        mut data: LogicCommand,
    ) -> anyhow::Result<bool> {
        match data {
            LogicCommand::Start => {
                log::debug!("新TCP连接:{:?},Addr:{:?}", self.stream, self.addr.ip());
                if self.haproxy_protocol {
                    // 下一阶段:haproxy 协议解析
                    api.send(LogicCommand::HaProxy)?;
                    return Ok(false);
                }
                // 下一阶段:初始化连接
                api.send(LogicCommand::ConnectionInit)?;
                return Ok(false);
            }
            LogicCommand::HaProxy => {
                let mut owned_stream: tokio::net::TcpStream = match self.stream.take() {
                    Some(stream) => stream,
                    None => return Ok(true), // 直接关闭
                };
                // let addr = self.haproxy_get_client_ip(&mut owned_stream).await;
                // log::info!("addr:{:?}",addr);
                self.stream = Some(owned_stream);
                // 下一阶段:初始化连接
                api.send(LogicCommand::ConnectionInit)?;
                return Ok(false);
            }
            LogicCommand::ConnectionInit => {
                // 你的连接不再属于你自己了
                let mut stream: tokio::net::TcpStream = match self.stream.take() {
                    Some(stream) => stream,
                    None => return Ok(true), // 直接关闭
                };
                let (rs, ws) = tokio::io::split(stream);
                let packet_socket = qexed_tcp_connect::PacketListener::new(
                    rs,
                    ws,
                    self.network_compression_threshold,
                );
                let (mut packet_read, mut packet_write) = packet_socket.split();
                // 拆分连接任务至
            }
            LogicCommand::Handshaking => {}
            LogicCommand::ListenClose(_) => {},
        }

        Ok(false)
    }
}
