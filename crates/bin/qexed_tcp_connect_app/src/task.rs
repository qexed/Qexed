use std::{collections::HashMap, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};

use async_trait::async_trait;
use chrono::Local;
use qexed_packet::{PacketCodec, PacketWriter, net_types::VarInt};
use qexed_protocol::to_server::status::{ping::Ping, ping_start::PingStart};
use qexed_task::{
    event::task::TaskEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage},
};
use rand::Rng;
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha1::{Digest, Sha1};
use tokio::{net::TcpStream, sync::{Mutex, mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}}, time::timeout};
use tokio_util::sync::CancellationToken;

use crate::messages::{ManagerCommand, TaskCommand};

#[derive(Debug)]
pub struct TcpConnectActor {
    socket: Option<TcpStream>,
    addr: std::net::SocketAddr,
    compression_threshold: usize,
    online_mode: bool,
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
    public_key_der: Vec<u8>,
    status_timeout_secs:i32,
}
impl TcpConnectActor {
    pub fn new(
        socket: TcpStream,
        addr: std::net::SocketAddr,
        compression_threshold: usize,
        online_mode: bool,
        private_key: RsaPrivateKey,
        public_key: RsaPublicKey,
        public_key_der: Vec<u8>,
        status_timeout_secs:i32,
    ) -> Self {
        Self {
            socket: Some(socket),
            addr,
            compression_threshold: compression_threshold,
            online_mode: online_mode,
            private_key,
            public_key,
            public_key_der,
            status_timeout_secs,
        }
    }
}
#[async_trait]
impl TaskEvent<ReturnMessage<TaskCommand>,ReturnMessage<ManagerCommand>> for TcpConnectActor {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        mut data: ReturnMessage<TaskCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskCommand::Start => {
                let api_clone = api.clone();
                let manage_api = manage_api.clone();
                let socket = self.socket.take();
                let addr: std::net::SocketAddr = self.addr.clone();
                let compression_threshold = self.compression_threshold.clone();
                let online_mode = self.online_mode.clone();
                let private_key: RsaPrivateKey = self.private_key.clone();
                let public_key: RsaPublicKey = self.public_key.clone();
                let public_key_der: Vec<u8> = self.public_key_der.clone();
                let status_timeout_secs = self.status_timeout_secs.clone();
                // let 
                tokio::spawn(async move {
                    if let Some(socket) = socket {
                        let api_clone = api_clone.clone();
                        let manage_api = manage_api.clone();
                        let (rs, ws) = tokio::io::split(socket);
                        // log::info!("新连接测试:{}",addr.clone());
                        let packet_socket =
                            qexed_tcp_connect::PacketListener::new(rs, ws, compression_threshold);
                        let (mut packet_read, mut packet_write) = packet_socket.split();
                        let mut qexed_logic_api = None;
                        let _: anyhow::Result<()> = async {
                            let set_protocol = qexed_tcp_connect::read_one_packet::<
                                qexed_protocol::to_server::handshaking::set_protocol::SetProtocol,
                            >(&mut packet_read)
                            .await?;
                            // log::info!("进服数据包内容:{:?}", set_protocol);
                            // 下阶段根据查询服务调用查询服务API
                        
                            if set_protocol.next_state.0 == 1 {
                                part_status(&mut packet_read, &mut packet_write, &manage_api,status_timeout_secs)
                                    .await?;
                                return Ok(());
                            } else if set_protocol.next_state.0 != 2 {
                                return Ok(());
                            }
                            // 检测协议版本号是否兼容:
                            if set_protocol.protocol_version.0 != qexed_config::PROTOCOL_VERSION {
                                // 下个阶段:但是服务端没写完
                                let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": format!("您的游戏版本与服务器版本不兼容\n目前服务器版本:{}",qexed_config::MC_VERSION),
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                                packet_write.send(server_info).await?;
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                return Ok(());
                            }
                        
                            // 登录阶段
                            let (player,logic_api) = login_status(&mut packet_read, &mut packet_write, &manage_api,compression_threshold,online_mode,private_key,public_key,public_key_der,addr.clone()).await?;
                            let logic_api = if let Some(api) = logic_api {
                                qexed_logic_api = Some(api.clone());
                                api // 将内部的 api 移出到变量 logic_api
                            } else {
                                // 处理 None 情况
                                return Ok(()); // 或进行其他清理工作后返回
                            };
                            // 登录阶段->配置阶段
                            // 建立直接连接(此阶段也会顺带检查客户端是否在线)
                            
                            // 登录阶段->配置阶段
                            // 注:我们会先直连 Game_Logic 服务进行配置阶段的处理
                            // pass
                            // 在 Game_Logic 配置阶段完成后
                            // 数据包读权限由 Game_Logic -> Game_Packet_Split 服务
                            // 写权限复制
                            // pass
                            // 读数据包流:
                            let (rpw, rpr) = unbounded_channel();
                            // 写数据包流
                            let (wpw, mut wpr) = unbounded_channel();
                            // 创建协调器
                            let shutdown = Arc::new(ConnectionShutdown::new());
                                                    
                            // 分离的写任务
                            let mut write_handle = {
                                let shutdown = Arc::clone(&shutdown);
                                tokio::spawn(async move {
                                    let mut packet_write = packet_write;
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
                            
                            if let Err(e) = packet_write.send_raw(pk).await {
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
                                    
                                    // 尝试优雅关闭写入端
                                    let _ = packet_write.shutdown().await;
                                    log::debug!("写任务退出完成");
                                })
                            };
                            
                            // 主循环处理读取
                            let mut read_handle = {
                                let shutdown = Arc::clone(&shutdown);
                                tokio::spawn(async move {
                                    let read_result = async {
                                        loop {
                                            tokio::select! {
                                                _ = shutdown.cancel_token.cancelled() => {
                                                    log::debug!("读任务收到取消信号");
                                                    break Ok(());
                                                }
                                                result = packet_read.read() => {
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
                            
                            // 发送初始任务到游戏逻辑
                            ReturnMessage::build(qexed_game_logic::message::TaskMessage::Start(player, Some(rpr), Some(wpw.clone())))
                                .get(&logic_api).await?;
                            ReturnMessage::build(qexed_game_logic::message::TaskMessage::Configuration(false))
                                .get(&logic_api).await?;
                            ReturnMessage::build(qexed_game_logic::message::TaskMessage::Play)
                                .get(&logic_api).await?;
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
                            Ok(())
                        }
                        .await;
                        if let Some(logic_api) = qexed_logic_api{
                            
                            let _ = ReturnMessage::build(qexed_game_logic::message::TaskMessage::Close)
                                .get(&logic_api)
                                .await;
                        }

                        
                        // 告知自己关闭自己
                        let _ = ReturnMessage::build(TaskCommand::ConnClose(addr))
                            .get(&api_clone)
                            .await;
                        anyhow::Ok(())
                    } else {
                        anyhow::Ok(())
                    }
                });
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            }
            TaskCommand::ConnClose(addr) => {
                // 向父级关闭告知自己
                ReturnMessage::build(ManagerCommand::ConnClose(addr))
                    .get(&manage_api)
                    .await?;
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
                return Ok(true);
            }
            TaskCommand::Shutdown(ref _why) => {
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(true);
            }

        }

        Ok(false)
    }
}
    pub fn kick_message_template(raw: String, context: HashMap<&str, String>) -> String {
        let mut result = raw;
        for (key, value) in context {
            result = result.replace(&format!("{{{}}}", key), &value);
        }
        result
    }
// 创建协调结构
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
async fn part_status(
    packet_read: &mut qexed_tcp_connect::PacketRead,
    packet_write: &mut qexed_tcp_connect::PacketSend,
    manage_api: &tokio::sync::mpsc::UnboundedSender<ReturnMessage<ManagerCommand>>,
    status_timeout_secs: i32,
) -> anyhow::Result<()> {
    // 将秒转换为 Duration
    let timeout_duration = Duration::from_secs(status_timeout_secs.max(0) as u64);
    
    loop {
        // 使用 timeout 包装读取操作
        let data_result = if timeout_duration.as_secs() > 0 {
            anyhow::Context::context(timeout(timeout_duration, packet_read.read())
                .await, "读取数据包超时")?
        } else {
            packet_read.read().await
        };
        
        let data = data_result?;
        let mut buf: bytes::BytesMut = bytes::BytesMut::new();
        buf.extend_from_slice(&data);
        let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
        let mut id: qexed_packet::net_types::VarInt = Default::default();
        id.deserialize(&mut reader)?;
        
        match id.0 {
            0x00 => {
                qexed_tcp_connect::decode_packet::<PingStart>(&mut reader)?;
                let return_data: ManagerCommand =
                    ReturnMessage::build(ManagerCommand::GetStatusPackageBytes(Default::default()))
                        .get(&manage_api)
                        .await?;
                
                match return_data {
                    ManagerCommand::GetStatusPackageBytes(value) => {
                        if let Some(value) = value {
                            packet_write.send_raw(value).await?;
                        }
                    }
                    _ => {}
                }
            }
            0x01 => {
                let pk = qexed_tcp_connect::decode_packet::<Ping>(&mut reader)?;
                let server_info = qexed_protocol::to_client::status::ping::Ping { time: pk.time };
                packet_write.send(server_info).await?;
            }
            _ => {}
        }
    }
}
async fn login_status(
    packet_read: &mut qexed_tcp_connect::PacketRead,
    packet_write: &mut qexed_tcp_connect::PacketSend,
    manage_api: &tokio::sync::mpsc::UnboundedSender<ReturnMessage<ManagerCommand>>,
    compression_threshold: usize,
    online_mode: bool,
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
    public_key_der: Vec<u8>,
    addr: std::net::SocketAddr
) -> anyhow::Result<(qexed_player::Player,Option<UnboundedSender<ReturnMessage<qexed_game_logic::message::TaskMessage>>>)> {
    let mut player: qexed_player::Player = Default::default();
    let mut verify_token: Option<[u8; 16]> = None;
    let mut encryption_started = false;
    let mut logic_api: Option<UnboundedSender<ReturnMessage<qexed_game_logic::message::TaskMessage>>>=None;
    loop {
        let data = packet_read.read().await?;
        let mut buf: bytes::BytesMut = bytes::BytesMut::new();
        buf.extend_from_slice(&data);
        let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
        let mut id: qexed_packet::net_types::VarInt = Default::default();
        id.deserialize(&mut reader)?;
        match id.0 {
            0x00 => {
                let pk = qexed_tcp_connect::decode_packet::<
                    qexed_protocol::to_server::login::login_start::LoginStart,
                >(&mut reader)?;
                
                player.username = pk.username;
                player.uuid = pk.player_uuid;
                // 检查有没有被拉黑
                if let ManagerCommand::LoginCheck(uuid,_ip,is_login,reason) = ReturnMessage::build(ManagerCommand::LoginCheck(player.uuid.clone(),Some(addr.ip()), false,None)).get(&manage_api).await?{
                    if uuid!=player.uuid.clone(){
                        let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                            reason: serde_json::json!({
                                "text": "玩家检查失败,疑似被篡改信息。请联系服务器管理员",
                                "color": "red",
                                "bold": true
                            }),
                        };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!("玩家检查失败,疑似被篡改信息。请联系服务器管理员"));
                    }
                    if is_login==false{
                        let reason = match reason{
                            Some(reason) => {
                                let mut context: HashMap<&str, String> = std::collections::HashMap::new();
                                context.insert("player", player.username.clone());
                                context.insert("ip", addr.ip().to_string());
                                context.insert("time", Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
                                kick_message_template(reason,context)
                            },
                            None=>"玩家登录频率过高,已禁止登录。".to_string()
                        };
                        let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                            reason: serde_json::json!({
                                "text": reason,
                                "color": "red",
                                "bold": true
                            }),
                        };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!(reason));
                    }
                }
                // 检查玩家是否在线
                if let ManagerCommand::CheckPlayeIsInList(uuid,is_login) = ReturnMessage::build(ManagerCommand::CheckPlayeIsInList(player.uuid.clone(), true)).get(&manage_api).await?{
                    if uuid!=player.uuid.clone(){
                        let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                            reason: serde_json::json!({
                                "text": "玩家检查失败,疑似被篡改信息。请联系服务器管理员",
                                "color": "red",
                                "bold": true
                            }),
                        };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!("玩家检查失败,疑似被篡改信息。请联系服务器管理员"));
                    }
                    if is_login==false{
                        let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                            reason: serde_json::json!({
                                "text": "当前玩家已登录服务器，请稍后再试",
                                "color": "red",
                                "bold": true
                            }),
                        };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!("当前玩家已登录服务器，请稍后再试"));
                    }
                } else {
                    let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": "玩家在线检查失败",
                            "color": "red",
                            "bold": true
                        }),
                    };
                    packet_write.send(server_info).await?;
                    return Err(anyhow::anyhow!("玩家在线检查失败"));
                }
                // 检查是否启用压缩
                if compression_threshold > 0 {
                    let server_info = qexed_protocol::to_client::login::compress::Compress {
                        threshold: VarInt(compression_threshold as i32),
                    };
                    packet_write.send(server_info).await?;
                    packet_read.set_compression(true);
                    packet_write.set_compression(true);
                }
                // 非online模式的话其实就结束了
                if !online_mode {
                    if let ManagerCommand::GetLogicApi(qexed_game_logic::message::ManagerMessage::NewPlayerConnect(uuid, is_true, err, logic_api2)) = ReturnMessage::build(ManagerCommand::GetLogicApi(
                        qexed_game_logic::message::ManagerMessage::NewPlayerConnect(
                            player.uuid.clone(), false, None,None)
                        )
                    ).get(&manage_api).await?{
                        if !is_true{
                            let server_info =
                                qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": "验证失败,疑似已在线",
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                            packet_write.send(server_info).await?;
                            return Err(anyhow::anyhow!("验证失败,疑似已在线"));
                        }
                        if let Some(err) = err{
                            let server_info =
                                qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": "玩家未离线",
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                            packet_write.send(server_info).await?;
                            return Err(err.into());
                        }
                        if let None = logic_api2{
                            let server_info =
                                qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": "玩家服务丢失，登录无效",
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                            packet_write.send(server_info).await?;
                            return Err(anyhow::anyhow!("玩家服务丢失，登录无效"));
                        }
                        logic_api =logic_api2;

                    } else {
                        let server_info =
                            qexed_protocol::to_client::login::disconnect::Disconnect {
                                reason: serde_json::json!({
                                    "text": "Api接口请求失败",
                                    "color": "red",
                                    "bold": true
                                }),
                            };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!("Api接口请求失败"));
                    }
                    packet_write
                        .send(qexed_protocol::to_client::login::success::Success {
                            uuid: player.uuid.clone(),
                            username: player.username.clone(),
                            properties: vec![],
                        })
                        .await?;
                    continue;
                }
                // 开始加密协商
                verify_token = Some(generate_verify_token());
                if let Some(verify_token) = verify_token {
                    let encryption_begin =
                        qexed_protocol::to_client::login::encryption_begin::EncryptionBegin {
                            server_id: String::new(), // 空字符串
                            public_key: public_key_der.clone(),
                            verify_token: verify_token.to_vec(),
                            should_authenticate: true,
                        };

                    packet_write.send(encryption_begin).await?;
                    encryption_started = true;
                } else {
                    let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": "加密认证启动失败,请联系管理员",
                            "color": "red",
                            "bold": true
                        }),
                    };
                    packet_write.send(server_info).await?;
                    return Err(anyhow::anyhow!("密钥认证启动失败"));
                }
            }
            0x01 => {
                if !encryption_started {
                    let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": "您尚未启用认证!!!",
                            "color": "red",
                            "bold": true
                        }),
                    };
                    packet_write.send(server_info).await?;
                    return Err(anyhow::anyhow!("未启用密钥认证进行了认证"));
                }
                let pk = qexed_tcp_connect::decode_packet::<
                    qexed_protocol::to_server::login::encryption_begin::EncryptionBegin,
                >(&mut reader)?;
                // 解密共享密钥
                let shared_secret = private_key
                    .decrypt(rsa::Pkcs1v15Encrypt, &pk.shared_secret)
                    .map_err(|e| anyhow::anyhow!("Failed to decrypt shared secret: {}", e));
                if shared_secret.is_err() {
                    let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": "非法密钥",
                            "color": "red",
                            "bold": true
                        }),
                    };
                    packet_write.send(server_info).await?;
                    return Err(anyhow::anyhow!("非法密钥"));
                }
                let shared_secret = shared_secret?;
                // 解密并验证令牌
                let received_token = private_key
                    .decrypt(rsa::Pkcs1v15Encrypt, &pk.verify_token)
                    .map_err(|e| anyhow::anyhow!("Failed to decrypt verify token: {}", e));
                if received_token.is_err() {
                    let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": "非法密钥,验证失败",
                            "color": "red",
                            "bold": true
                        }),
                    };
                    packet_write.send(server_info).await?;
                    return Err(anyhow::anyhow!("非法密钥,验证失败"));
                }
                let received_token = received_token?;

                if let Some(verify_token) = verify_token {
                    if received_token != verify_token.to_vec() {
                        let server_info =
                            qexed_protocol::to_client::login::disconnect::Disconnect {
                                reason: serde_json::json!({
                                    "text": "验证令牌丢失",
                                    "color": "red",
                                    "bold": true
                                }),
                            };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!("Verify token mismatch"));
                    }
                    // 启用加密
                    if shared_secret.len() >= 16 {
                        let aes_key: &[u8] = &shared_secret[..16];
                        packet_read.set_encryption(aes_key)?;
                        packet_write.set_encryption(aes_key)?;
                    } else {
                        return Err(anyhow::anyhow!("Shared secret too short"));
                    }
                    // 下一步是登录认证
                    // 使用Java特有的哈希算法                    
                    let result = calculate_server_hash("",&shared_secret,&public_key_der);
                    let is_true = qexed_mojang_auth::handle_login(player.username.clone(),Some(addr),result).await;
                    if let Err(err) = is_true{
                        let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                            reason: serde_json::json!({
                                "text": format!("{}",err),
                                "color": "red",
                                "bold": true
                            }),
                        };
                        packet_write.send(server_info).await?;
                        return Err(err.into());
                    }
                    let return_data = is_true?;
                    player.username = return_data.1;
                    player.uuid = return_data.0;
                    if let ManagerCommand::GetLogicApi(qexed_game_logic::message::ManagerMessage::NewPlayerConnect(uuid, is_true, err, logic_api2)) = ReturnMessage::build(ManagerCommand::GetLogicApi(
                        qexed_game_logic::message::ManagerMessage::NewPlayerConnect(
                            player.uuid.clone(), false, None,None)
                        )
                    ).get(&manage_api).await?{
                        if !is_true{
                            let server_info =
                                qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": "验证失败,疑似已在线2",
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                            packet_write.send(server_info).await?;
                            return Err(anyhow::anyhow!("验证失败,疑似已在线2"));
                        }
                        if let Some(err) = err{
                            let server_info =
                                qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": "玩家未离线",
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                            packet_write.send(server_info).await?;
                            return Err(err.into());
                        }
                        if let None = logic_api2{
                            let server_info =
                                qexed_protocol::to_client::login::disconnect::Disconnect {
                                    reason: serde_json::json!({
                                        "text": "玩家服务丢失，登录无效",
                                        "color": "red",
                                        "bold": true
                                    }),
                                };
                            packet_write.send(server_info).await?;
                            return Err(anyhow::anyhow!("玩家服务丢失，登录无效"));
                        }
                        logic_api =logic_api2;

                    } else {
                        let server_info =
                            qexed_protocol::to_client::login::disconnect::Disconnect {
                                reason: serde_json::json!({
                                    "text": "Api接口请求失败",
                                    "color": "red",
                                    "bold": true
                                }),
                            };
                        packet_write.send(server_info).await?;
                        return Err(anyhow::anyhow!("Api接口请求失败"));
                    }
                    let mut properties: Vec<qexed_protocol::to_client::login::success::Properties> = vec![];
                    for i in return_data.2 {
                        properties.push(qexed_protocol::to_client::login::success::Properties{
                            name: i.name,
                            value: i.value,
                            signature: i.signature,
                        });
                    }
                    player.properties = properties.clone();
                    packet_write
                        .send(qexed_protocol::to_client::login::success::Success {
                            uuid: player.uuid.clone(),
                            username: player.username.clone(),
                            properties: properties,
                        })
                        .await?;

                } else {
                    let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                        reason: serde_json::json!({
                            "text": "验证令牌丢失",
                            "color": "red",
                            "bold": true
                        }),
                    };
                    packet_write.send(server_info).await?;
                    return Err(anyhow::anyhow!("验证令牌丢失"));
                }
            }
            0x02 => {
                let pk = qexed_tcp_connect::decode_packet::<
                    qexed_protocol::to_server::login::login_plugin_response::LoginPluginResponse,
                >(&mut reader)?;
                log::debug!("{:?}",pk);
            }
            0x03 => {
                qexed_tcp_connect::decode_packet::<
                    qexed_protocol::to_server::login::login_acknowledged::LoginAcknowledged,
                >(&mut reader)?;
                // 检查玩家是否在线(其实是获取Api接口)
                if let None = logic_api{
                    return Err(anyhow::anyhow!("登录失败,疑似玩家已上线"));
                }
                return Ok((player,logic_api))

            }
            0x04 => {
                let pk = qexed_tcp_connect::decode_packet::<
                    qexed_protocol::to_server::login::cookie_response::CookieResponse,
                >(&mut reader)?;
            }
            _ => {}
        }
    }
}

pub fn generate_verify_token() -> [u8; 16] {
    let mut token = [0u8; 16];
    rand::thread_rng().fill(&mut token);
    token
}
pub fn calculate_server_hash(server_id: &str, shared_secret: &[u8], public_key: &[u8]) -> String {
    use sha1::{Sha1, Digest};
    
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key);
    
    let hash = hasher.finalize();
    let hex = hex::encode(hash);
    
    if (hash[0] & 0x80) != 0 {
        format!("-{}", hex.trim_start_matches("ff"))
    } else {
        hex
    }
}