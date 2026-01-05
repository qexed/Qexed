use std::{net::{IpAddr, SocketAddr}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};

use anyhow::Context;
use bytes::BytesMut;
use qexed_config::app::qexed_tcp_connect_app::ForwardingMode;
use qexed_packet::{Packet, PacketCodec, PacketReader};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::{mpsc::{UnboundedReceiver, UnboundedSender}, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    logic_task::{read_task::ReadTask, write_task::WriteTask},
    messages::{LogicCommand, ManagerCommand, ReadCommand, WriteCommand},
};
pub mod bc;
pub mod victory;
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
    proxy_is_login:bool,
    haproxy_protocol: bool,
    packet_read: Option<MessageSender<ReadCommand>>,
    packet_send: Option<MessageSender<WriteCommand>>,
    is_run_close: bool,
    status_timeout_secs: i32,
    part:u8,
    server_host:String,
    client_ip:IpAddr,
    player: qexed_player::Player,
    player_uuid:Uuid,
    player_name:String,
    player_properties:Vec<qexed_protocol::to_client::login::success::Properties>,
    logic_api: Option<UnboundedSender<ReturnMessage<qexed_game_logic::message::TaskMessage>>>
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
            proxy_is_login:false,
            haproxy_protocol,
            player:Default::default(),
            packet_read: None,
            packet_send: None,
            is_run_close: false,
            status_timeout_secs,
            part:0,
            server_host:String::new(),
            client_ip:addr.ip(),
            player_uuid:Default::default(),
            player_name:Default::default(),
            player_properties:vec![],
            logic_api:None,
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
    pub async fn set_compression(&self,is_use:bool,
        packet_send: &tokio::sync::mpsc::UnboundedSender<WriteCommand>,
        packet_read: &tokio::sync::mpsc::UnboundedSender<ReadCommand>,
    )-> anyhow::Result<()>{
        let (ss,sr) = oneshot::channel();
        let (rs,rr) = oneshot::channel();
        packet_send.send(WriteCommand::SetCompression(
            is_use,ss
        ))?;
        sr.await?;
        packet_read.send(ReadCommand::SetCompression(
            is_use,rs
        ))?;
        rr.await?;
        Ok(())
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
        match self.part {
            1=>{
                let server_info =
                    qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": text,
                            "color": "red",
                            "bold": true
                        }),
                    };
                self.send_packet(packet_send, server_info).await?;
            }
            // 2=>{
            //     let server_info =
            //         qexed_protocol::to_client::configuration::disconnect::Disconnect {
            //             reason: serde_json::json!({
            //                 "text": text,
            //                 "color": "red",
            //                 "bold": true
            //             }),
            //         };
            //     self.send_packet(packet_send, server_info).await?;
            // }
            // 3=>{
            //     let server_info =
            //         qexed_protocol::to_client::play::disconnect::Disconnect {
            //             reason: serde_json::json!({
            //                 "text": text,
            //                 "color": "red",
            //                 "bold": true
            //             }),
            //         };
            //     self.send_packet(packet_send, server_info).await?; 
            // }
            _=>{}
        }

        Ok(())
    }
    pub fn get_player_uuid(&self)->Option<uuid::Uuid>{
        if self.player_uuid.is_nil(){
            return None;
        } else {
            return Some(self.player_uuid);
        }
    }
}

#[async_trait::async_trait]
impl TaskEvent<LogicCommand, ReturnMessage<ManagerCommand>> for LogicTask {
    async fn event(
        &mut self,
        api: &MessageSender<LogicCommand>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: LogicCommand,
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
                
                match set_protocol.next_state.0 {
                    1 => {
                        api.send(LogicCommand::Status(set_protocol))?;
                    }
                    2 => {
                        self.part = 1;
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
                        self.part = 1;
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
                            self.handle_bungeecord_properties(i).await?;
                        }
                        _=>{
                            return Err(anyhow::anyhow!("非法BC字段"))
                        }
                    }
                }
                api.send(LogicCommand::Login(set_protocol))?;
            }
            LogicCommand::Login(set_protocol) => {
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
                            let pk = qexed_tcp_connect::decode_packet::<qexed_protocol::to_server::login::login_start::LoginStart>(&mut reader)?;
                            if self.proxy==false{
                                self.player_uuid = pk.player_uuid;// 不信任客户端自己的UUID,因为代理已经提供权威的了
                                self.player_name = pk.username.clone();
                            }
                            // 检测是否在黑名单
                            if let ManagerCommand::CheckIsInBlockList(_,ban) = ReturnMessage::build(ManagerCommand::CheckIsInBlockList(self.player_uuid.clone(), None)).get(&manage_api).await?{
                                if let Some(ban) = ban{
                                    return Err(anyhow::anyhow!(ban));
                                }
                            } else {
                                return Err(anyhow::anyhow!("黑名单检查失败,请联系管理员"));
                            }
                            // 检查是否在白名单(如果启用了)
                            if let ManagerCommand::CheckIsInWhiteList(_,ban) = ReturnMessage::build(ManagerCommand::CheckIsInWhiteList(self.player_uuid.clone(), None)).get(&manage_api).await?{
                                if let Some(ban) = ban{
                                    return Err(anyhow::anyhow!(ban));
                                }
                            } else {
                                return Err(anyhow::anyhow!("黑名单检查失败,请联系管理员"));
                            }
                            
                            // 代理模式检查
                            if self.proxy{
                                match self.proxy_protocol {
                                    ForwardingMode::QTunnel=>{
                                        return Err(anyhow::anyhow!("QTunnel自己都还没出来,支持个屁代理协议"));
                                    },
                                    ForwardingMode::Victory=>{
                                        self.send_packet(packet_send,qexed_protocol::to_client::login::login_plugin_request::LoginPluginRequest{
                                            message_id: qexed_packet::net_types::VarInt(0),
                                            channel: "velocity:player_info".to_string(),
                                            data: qexed_packet::net_types::RestBuffer(vec![4]),
                                        }).await?;
                                        tokio::time::sleep(Duration::from_millis(100)).await;
                                        
                                        let login_plugin_response = self.read_the_packet::<qexed_protocol::to_server::login::login_plugin_response::LoginPluginResponse>(packet_read).await?;
                                        if let Some(data) = login_plugin_response.data{
                                            let data = data.0;
                                            let (signature, data_without_signature) = data.split_at(32);
                                            if !victory::check_integrity((signature, data_without_signature), &self.proxy_token) {
                                                return Err(victory::VelocityError::FailedVerifyIntegrity.into());
                                            }

                                            let mut buf = BytesMut::new();
                                            buf.extend_from_slice(&data_without_signature);
                                            let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
                                            let version: qexed_packet::net_types::VarInt = Default::default();
                                            id.deserialize(&mut reader)?;
                                            // Check velocity version
                                            let version = version.0 as u8;
                                            if version > 4 {
                                                return Err(victory::VelocityError::UnsupportedForwardVersion(
                                                    version,
                                                    4,
                                                ).into());
                                            }
                                            let mut addr2:String = Default::default();
                                            addr2.deserialize(&mut reader)?;
                                            let socket_addr: SocketAddr = SocketAddr::new(
                                                addr2.parse::<IpAddr>()
                                                    .map_err(|_| victory::VelocityError::FailedParseAddress)?,
                                                self.addr.port(),
                                            );
                                            let mut buf = bytes::BytesMut::new();
                                            buf.extend_from_slice(&data_without_signature);
                                            let mut reader = PacketReader::new(Box::new(&mut buf));
                                            // self.player_uuid.deserialize(&mut reader)?;
                                            // self.player_name.deserialize(&mut reader)?;
                                            // self.player_properties.deserialize(&mut reader)?;
                                            // 暂时性的,后续支持
                                            self.player_uuid = pk.player_uuid;// 不信任客户端自己的UUID,因为代理已经提供权威的了
                                            self.player_name = pk.username.clone();
                                            self.proxy_is_login = true
                                        } else {
                                            return Err(anyhow::anyhow!("代理数据包错误"));
                                        }
                                    },
                                    ForwardingMode::BungeeCord|ForwardingMode::BungeeGuard=>{
                                        return Err(anyhow::anyhow!("BC代理协议暂时不支持,后续兼容"));
                                    },
                                }
                            }
                            // 检查是否启用压缩
                            if self.network_compression_threshold>0{
                                let server_info = qexed_protocol::to_client::login::compress::Compress {
                                    threshold: qexed_packet::net_types::VarInt(self.network_compression_threshold as i32),
                                };
                                self.send_packet(packet_send, server_info).await?;
                                self.set_compression(true,packet_send,packet_read).await?;
                            }
                            // 非online模式
                            if !self.online_mode || self.proxy_is_login{
                                // 在线检查
                                if let ManagerCommand::CheckPlayeIsOnline(_,is_online) = ReturnMessage::build(ManagerCommand::CheckPlayeIsOnline(self.player_uuid.clone(), false)).get(manage_api).await?{
                                    if is_online{
                                        // 使用如下理由踢出玩家下线:您已在异地登录
                                        ReturnMessage::build(ManagerCommand::KickPlayer(self.player_uuid, "您的账号在另一处登录".to_string())).get(
                                            manage_api
                                        ).await?;
                                    }
                                } else {
                                    return Err(anyhow::anyhow!("玩家在线检查失败,请联系管理员"));
                                }
                                // 登录阶段完成->配置阶段
                                if let ManagerCommand::GetLogicApi(qexed_game_logic::message::ManagerMessage::NewPlayerConnect(uuid, is_true, err, logic_api2)) = ReturnMessage::build(ManagerCommand::GetLogicApi(
                                    qexed_game_logic::message::ManagerMessage::NewPlayerConnect(
                                        self.player_uuid.clone(), false, None,None)
                                    )
                                ).get(manage_api).await?{
                                    if !is_true{
                                        return Err(anyhow::anyhow!("验证失败,疑似已在线"));
                                    }
                                    if let Some(err) = err{
                                        return Err(anyhow::anyhow!("验证失败,疑似已在线"));
                                    }
                                    if let None = logic_api2{
                                        return Err(anyhow::anyhow!("玩家服务丢失，登录无效"));
                                    }
                                    self.logic_api =logic_api2;
                                
                                } else {
                                    return Err(anyhow::anyhow!("玩家服务Api接口请求失败"));
                                }
                                // 检查玩家是否在线(其实是获取Api接口)
                                self.send_packet(packet_send,
                                    qexed_protocol::to_client::login::success::Success {
                                        uuid: self.player_uuid.clone(),
                                        username: self.player_name.clone(),
                                        properties: self.player_properties.clone(),
                                    }
                                ).await?;
                                self.part = 2;
                                self.player.username = self.player_name.clone();
                                self.player.uuid = self.player_uuid.clone();
                                self.player.properties = self.player_properties.clone();
                                continue;
                            }
                            return Err(anyhow::anyhow!("BC代理协议暂时不支持,后续兼容"));
                        }
                        0x03 => {
                            qexed_tcp_connect::decode_packet::<
                                qexed_protocol::to_server::login::login_acknowledged::LoginAcknowledged,
                            >(&mut reader)?;
                            api.send(LogicCommand::Finish)?;
                            return Ok(false);
                        }
                        _ => {

                        }
                    }
                }

                
                        
            }
            LogicCommand::Finish=>{
                
                let logic_api = if let Some(api) = &self.logic_api {
                    api.clone()
                } else {
                    // 处理 None 情况
                    return Err(anyhow::anyhow!("下阶段游戏服务不可用")); // 或进行其他清理工作后返回
                };
                let packet_read = match &self.packet_read {
                    Some(v) => v,
                    None => return Ok(true),
                };
                let packet_send = match &self.packet_send {
                    Some(v) => v,
                    None => return Ok(true),
                };
                let packet_send2 = packet_send.clone();
                let packet_read2 = packet_read.clone();
                let player2 = self.player.clone();
                let api2 =api.clone();
                tokio::spawn(async move {
                    let player = player2;
                    // 由于接口兼容性,读写任务还得创建。
                    // 读数据包流:
                    let (rpw, rpr) = tokio::sync::mpsc::unbounded_channel();
                    // 写数据包流
                    let (wpw, mut wpr) = tokio::sync::mpsc::unbounded_channel();
                    // 创建协调器
                    let shutdown = Arc::new(ConnectionShutdown::new());
                    // 分离的写任务
                    let mut write_handle = {
                    let shutdown = Arc::clone(&shutdown);
                    
                    tokio::spawn(async move {
                        let packet_send = packet_send2;
                        loop {
                            tokio::select! {
                                _ = shutdown.cancel_token.cancelled() => {
                                    log::debug!("写任务收到取消信号");
                                    break;
                                }
                                pk = wpr.recv() => {
                                    match pk {
                                        Some(pk) => {
                                            // 检查读任务是否已停止
                                            if !shutdown.should_write() {
                                                log::debug!("读任务已停止，写任务退出");
                                                break;
                                            }
                                            
                                            if let Err(e) = packet_send.send(WriteCommand::RawPacket(pk)) {
                                                log::error!("写入数据包出错: {}", e);
                                                break;
                                            }
                                        }
                                        None => {
                                            // 发送端关闭
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        
                        // 标记写任务已停止
                        shutdown.write_stopped.store(true, Ordering::SeqCst);
                    })
                    };

                    // 主循环处理读取
                    let mut read_handle = {
                    let shutdown = Arc::clone(&shutdown);
                    
                    tokio::spawn(async move {
                        let read_result = async {
                            loop {
                                let (w, r) = oneshot::channel();
                                packet_read2.send(ReadCommand::RawPacket(w))?;
                                tokio::select! {
                                    _ = shutdown.cancel_token.cancelled() => {
                                        log::debug!("读任务收到取消信号");
                                        break Ok(());
                                    }
                                    result = r => {
                                        // 检查写任务是否已停止
                                        if !shutdown.should_read() {
                                            log::debug!("写任务已停止，读任务退出");
                                            break Ok(());
                                        }
                                    
                                        match result {
                                            Ok(pk) => {
                                                if rpw.is_closed() {
                                                    break Ok(());
                                                }
                                                if let Err(e) = rpw.send(pk) {
                                                    log::error!("发送数据包到通道出错: {}", e);
                                                    break Err(anyhow::anyhow!("通道发送失败: {}", e));
                                                }
                                            }
                                            Err(e) => {
                                                // 读取失败，立即通知写任务
                                                shutdown.cancel_token.cancel();
                                                break Err(anyhow::anyhow!("读取数据包失败: {}", e));
                                            }
                                        }
                                    }
                                }
                            }
                        }.await;
                        
                        // 标记读任务已停止
                        shutdown.read_stopped.store(true, Ordering::SeqCst);
                        
                        read_result
                    })
                    };
                    
                
                
                    // let api = api2;
                    
                    // 发送初始任务到游戏逻辑
                    ReturnMessage::build(qexed_game_logic::message::TaskMessage::Start(player, Some(rpr), Some(wpw.clone())))
                        .get(&logic_api).await?;
                    
                    match ReturnMessage::build(qexed_game_logic::message::TaskMessage::Configuration(false))
                        .get(&logic_api).await{
                            Ok(_v)=>{},
                            Err(_v)=>{
                            let _ = ReturnMessage::build(qexed_game_logic::message::TaskMessage::Close)
                                .get(&logic_api)
                                .await;
                            }
                        };
                    
                    match ReturnMessage::build(qexed_game_logic::message::TaskMessage::Play)
                        .get(&logic_api).await{
                            Ok(_v)=>{},
                            Err(_v)=>{
                            let _ = ReturnMessage::build(qexed_game_logic::message::TaskMessage::Close)
                                .get(&logic_api)
                                .await;
                            }
                        };
                    // 等待任一任务完成，然后协调关闭
                    tokio::select! {
                        read_result = &mut read_handle => {
                            match read_result {
                                Ok(Ok(())) => log::debug!("读任务正常结束"),
                                Ok(Err(e)) => log::error!("读任务出错: {}", e),
                                Err(join_err) => log::error!("读任务panic: {}", join_err),
                            }
                            // 无论读任务如何结束，都启动关闭流程
                            shutdown.shutdown();
                        }
                        write_result = &mut write_handle => {
                            match write_result {
                                Ok(()) => log::debug!("写任务正常结束"),
                                Err(join_err) => log::error!("写任务panic: {}", join_err),
                            }
                            // 写任务结束，也启动关闭
                            shutdown.shutdown();
                        }
                    }
                    
                    // 等待另一个任务结束（最多等5秒）
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                            log::warn!("等待任务结束超时，强制关闭");
                        }
                        _ = async {
                            if !shutdown.read_stopped.load(Ordering::SeqCst) {
                                let _ = read_handle.await;
                            }
                            if !shutdown.write_stopped.load(Ordering::SeqCst) {
                                let _ = write_handle.await;
                            }
                        } => {}
                    }
                    
                    // 清理通道
                    drop(wpw);
                    log::debug!("连接处理完全退出");
                    
                    let (w,r) = oneshot::channel();
                    let _ = api2.send(LogicCommand::Close("客户端断开服务器".to_string(),w));
                    anyhow::Ok(())
                });
                
            }
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
                    if let Some(logic_api) = &self.logic_api{
                        let _ = ReturnMessage::build(qexed_game_logic::message::TaskMessage::Close)
                            .get(&logic_api)
                            .await;
                    }
                    let _ = ReturnMessage::build(ManagerCommand::TaskClose(self.addr,self.get_player_uuid()))
                        .get(&manage_api)
                        .await;
                    
                    return Ok(true);
                }
            }
            LogicCommand::Close(close_why,s) => {
                if let Some(logic_api) = &self.logic_api{
                    let _ = ReturnMessage::build(qexed_game_logic::message::TaskMessage::Close)
                        .get(&logic_api)
                        .await;
                }
                if let Some(sub_task_api) = &self.packet_send {

                    let _ = self.disconnect(sub_task_api, close_why).await;

                    let _ = sub_task_api.send(WriteCommand::Close);
                }
                // 执行关闭命令
                if let Some(sub_task_api) = &self.packet_read {
                    let _ = sub_task_api.send(ReadCommand::Close);
                }
                let _ = ReturnMessage::build(ManagerCommand::TaskClose(self.addr,self.get_player_uuid()))
                    .get(&manage_api)
                    .await;
                let _ = s.send(());
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
            let close_why = match self.other.event(&api, &manage_api, data).await {
                Ok(false)=>continue,
                Ok(true)=>"".to_string(),
                Err(err)=>err.to_string(),

            };
            let (w,r) = oneshot::channel();
            let _ = self
                .other
                .event(&api, &manage_api, LogicCommand::Close(close_why,w))
                .await;
            let _ = r.await;
            receiver.close();
            break;
        }
        Ok(())
    }
}
struct ConnectionShutdown {
    cancel_token: CancellationToken,
    read_stopped: Arc<AtomicBool>,
    write_stopped: Arc<AtomicBool>,
}

impl ConnectionShutdown {
    fn new() -> Self {
        Self {
            cancel_token: CancellationToken::new(),
            read_stopped: Arc::new(AtomicBool::new(false)),
            write_stopped: Arc::new(AtomicBool::new(false)),
        }
    }
    
    // 通知双方关闭
    fn shutdown(&self) {
        self.cancel_token.cancel();
    }
    
    // 检查是否应该继续读取
    fn should_read(&self) -> bool {
        !self.write_stopped.load(Ordering::SeqCst)
    }
    
    // 检查是否应该继续写入
    fn should_write(&self) -> bool {
        !self.read_stopped.load(Ordering::SeqCst)
    }
}