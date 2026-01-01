use std::net::IpAddr;

use anyhow::Context;
use qexed_packet::{Packet, PacketCodec};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::{mpsc::UnboundedReceiver, oneshot};
use uuid::Uuid;

use crate::{
    logic_task::{read_task::ReadTask, write_task::WriteTask},
    messages::{LogicCommand, ManagerCommand, ReadCommand, WriteCommand},
};
pub mod bc;
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
    packet_read: Option<MessageSender<ReadCommand>>,
    packet_send: Option<MessageSender<WriteCommand>>,
    is_run_close: bool,
    status_timeout_secs: i32,
    part:u8,
    server_host:String,
    client_ip:IpAddr,
    player_uuid:Uuid,
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
        status_timeout_secs: i32,
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
            packet_read: None,
            packet_send: None,
            is_run_close: false,
            status_timeout_secs,
            part:0,
            server_host:String::new(),
            client_ip:addr.ip(),
            player_uuid:Default::default(),
        }
    }
    pub async fn read_one_packet(
        &self,
        packet_read: &tokio::sync::mpsc::UnboundedSender<ReadCommand>,
    ) -> anyhow::Result<Vec<u8>> {
        let (w, r) = oneshot::channel();
        packet_read.send(ReadCommand::RawPacket(w))?;
        Ok(r.await?)
    }
    pub async fn read_the_packet<T: Packet>(
        &self,
        packet_read: &tokio::sync::mpsc::UnboundedSender<ReadCommand>,
    ) -> anyhow::Result<T> {
        let data = self.read_one_packet(packet_read).await?;
        Ok(qexed_tcp_connect::read_one_packet_byvec(data).await?)
    }
    pub async fn send_packet<T: Packet>(
        &self,
        packet_send: &tokio::sync::mpsc::UnboundedSender<WriteCommand>,
        pk: T,
    ) -> anyhow::Result<()> {
        Ok(packet_send.send(WriteCommand::RawPacket(
            PacketSend::build_send_packet(pk).await?,
        ))?)
    }
    pub async fn send_raw_packet(
        &self,
        packet_send: &tokio::sync::mpsc::UnboundedSender<WriteCommand>,
        pk: bytes::Bytes,
    ) -> anyhow::Result<()> {
        Ok(packet_send.send(WriteCommand::RawPacket(pk))?)
    }
    pub async fn disconnect(
        &self,
        packet_send: &tokio::sync::mpsc::UnboundedSender<WriteCommand>,
        text: String,
    ) -> anyhow::Result<()> {
        let server_info =
            qexed_protocol::to_client::login::disconnect::Disconnect {
                reason: serde_json::json!({
                    "text": text,
                    "color": "red",
                    "bold": true
                }),
            };
        self.send_packet(packet_send, server_info).await?;
        Ok(())
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
                let owned_stream: tokio::net::TcpStream = match self.stream.take() {
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
                let stream: tokio::net::TcpStream = match self.stream.take() {
                    Some(stream) => stream,
                    None => return Ok(true), // 直接关闭
                };
                let (rs, ws) = tokio::io::split(stream);
                let packet_socket = qexed_tcp_connect::PacketListener::new(
                    rs,
                    ws,
                    self.network_compression_threshold,
                );
                let (packet_read, packet_write) = packet_socket.split();
                // 拆分连接任务至子任务
                let (task, task_send) = read_task::TaskFinish::new(
                    api.clone(),
                    ReadTask::new(self.addr.clone(), packet_read),
                );
                task.run().await?;
                task_send.send(ReadCommand::Start)?;
                self.packet_read = Some(task_send);
                let (task, task_send) = write_task::TaskFinish::new(
                    api.clone(),
                    WriteTask::new(self.addr.clone(), packet_write),
                );
                task.run().await?;
                task_send.send(WriteCommand::Start)?;
                self.packet_send = Some(task_send);
                // 读写任务分割完成,下一阶段:心跳包
                api.send(LogicCommand::Handshaking)?;
            }
            LogicCommand::Handshaking => {
                let packet_read = match &self.packet_read {
                    Some(v) => v,
                    None => return Ok(true),
                };
                let packet_send = match &self.packet_send {
                    Some(v) => v,
                    None => return Ok(true),
                };
                let set_protocol: qexed_protocol::to_server::handshaking::set_protocol::SetProtocol = self.read_the_packet::<qexed_protocol::to_server::handshaking::set_protocol::SetProtocol>(packet_read).await?;
                self.server_host = set_protocol.server_host.clone();
                self.part = 1;
                match set_protocol.next_state.0 {
                    1 => {
                        api.send(LogicCommand::Status(set_protocol))?;
                    }
                    2 => {
                        if self.proxy{
                            if self.proxy_protocol == qexed_config::app::qexed_tcp_connect_app::ForwardingMode::BungeeCord || 
                               self.proxy_protocol == qexed_config::app::qexed_tcp_connect_app::ForwardingMode::BungeeGuard
                            {
                                api.send(LogicCommand::BungeeCordProxyHandshaking(set_protocol))?;
                                return Ok(false);
                            }
                        }
                        api.send(LogicCommand::Login(set_protocol))?;
                    }
                    _ => {
                        self.disconnect(packet_send,format!("您的游戏版本与服务器版本不兼容\n目前服务器版本:{}",qexed_config::MC_VERSION)).await?;
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            LogicCommand::Status(_set_protocol) => {
                let packet_read = match &self.packet_read {
                    Some(v) => v,
                    None => return Ok(true),
                };
                let packet_send = match &self.packet_send {
                    Some(v) => v,
                    None => return Ok(true),
                };
                let timeout_duration =
                    std::time::Duration::from_secs(self.status_timeout_secs.max(0) as u64);
                loop {
                    let data = if timeout_duration.as_secs() > 0 {
                        anyhow::Context::context(
                            tokio::time::timeout(
                                timeout_duration,
                                self.read_one_packet(packet_read),
                            )
                            .await,
                            "读取数据包超时",
                        )?
                    } else {
                        self.read_one_packet(packet_read).await
                    }?;
                    let mut buf: bytes::BytesMut = bytes::BytesMut::new();
                    buf.extend_from_slice(&data);
                    let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
                    let mut id: qexed_packet::net_types::VarInt = Default::default();
                    id.deserialize(&mut reader)?;
                    
                    match id.0 {
                        0x00 => {
                            qexed_tcp_connect::decode_packet::<qexed_protocol::to_server::status::ping_start::PingStart>(&mut reader)?;
                            if let ManagerCommand::GetStatusPackageBytes(Some(value)) = ReturnMessage::build(
                                ManagerCommand::GetStatusPackageBytes(Default::default()),

                            ).get(&manage_api).await?{
                                self.send_raw_packet(packet_send, value).await?;
                            } else {
                                return Ok(true);
                            }
                        }
                        0x01 => {
                            let pk = qexed_tcp_connect::decode_packet::<qexed_protocol::to_server::status::ping::Ping>(&mut reader)?;
                            let server_info =
                                qexed_protocol::to_client::status::ping::Ping { time: pk.time };
                            self.send_packet(packet_send, server_info).await?;
                        }
                        _ => {
                            return Ok(true);
                        }
                    }
                }
            }
            LogicCommand::BungeeCordProxyHandshaking(set_protocol) => {
                // 切割字符串
                let text = set_protocol.server_host.split("\0");
                // log::info!("set_protocol:{:?}",set_protocol);
                self.server_host = set_protocol.server_host.clone();
                // log::info!("text:{:?}",text);
                let mut l = 0;
                for i in text{
                    l+=1;
                    match l {
                        1=>{
                            self.server_host = i.to_string();
                        },
                        2=>{
                            self.client_ip = i.parse().context("客户端ip解析失败")?;
                        }
                        3=>{
                            self.player_uuid = i.parse().context("玩家uuid解析失败")?;
                        }
                        4=>{
                            self.handle_bungeecord_properties(i).await.context("BC代理心跳包数组解析失败")?;
                        }
                        _=>{
                            return Err(anyhow::anyhow!("非法BC字段"))
                        }
                    }
                }
                api.send(LogicCommand::Login(set_protocol))?;
            }
            LogicCommand::Login(set_protocol) => {}
            LogicCommand::ListenClose(is_read) => {
                if !self.is_run_close {
                    if is_read {
                        if let Some(sub_task_api) = &self.packet_send {
                            let _ = sub_task_api.send(WriteCommand::Close);
                        }
                    } else {
                        if let Some(sub_task_api) = &self.packet_read {
                            let _ = sub_task_api.send(ReadCommand::Close);
                        }
                    }
                    self.is_run_close = true
                } else {
                    let _ = ReturnMessage::build(ManagerCommand::TaskClose(self.addr))
                        .get(&manage_api)
                        .await;
                    return Ok(true);
                }
            }
            LogicCommand::Close => {
                // 执行关闭命令
                if let Some(sub_task_api) = &self.packet_read {
                    let _ = sub_task_api.send(ReadCommand::Close);
                }
                if let Some(sub_task_api) = &self.packet_send {
                    let _ = sub_task_api.send(WriteCommand::Close);
                }
                let _ = ReturnMessage::build(ManagerCommand::TaskClose(self.addr))
                    .get(&manage_api)
                    .await;
                return Ok(true);
            }
        }

        Ok(false)
    }
}
pub struct TaskFinish {
    api: MessageSender<LogicCommand>,
    manage_api: MessageSender<ReturnMessage<ManagerCommand>>,
    other: LogicTask,
    receiver: Option<UnboundedReceiver<LogicCommand>>,
}
impl TaskFinish {
    pub fn new(
        manage_api: MessageSender<ReturnMessage<ManagerCommand>>,
        data: LogicTask,
    ) -> (Self, MessageSender<LogicCommand>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                manage_api: manage_api,
                other: data,
                receiver: Some(r),
            },
            w,
        )
    }
    // 请注意:下面的所有权转移并不是失误,是刻意的设计
    pub async fn run(self) -> anyhow::Result<()> {
        tokio::spawn(self.listen());
        Ok(())
    }
    async fn listen(mut self) -> anyhow::Result<()> {
        let mut receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("接收管道不存在"))?;
        let api = self.api;
        let manage_api = self.manage_api;
        while let Some(data) = receiver.recv().await {
            // 这里我们后面修改来实现具体业务逻辑
            if let Ok(is_true) = self.other.event(&api, &manage_api, data).await {
                if !is_true {
                    continue;
                }
            }
            let _ = self
                .other
                .event(&api, &manage_api, LogicCommand::Close)
                .await;
            receiver.close();
        }
        Ok(())
    }
}
