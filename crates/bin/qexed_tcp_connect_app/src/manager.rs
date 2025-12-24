use crate::{messages::{ManagerCommand, TaskCommand}, task::TcpConnectActor};
use anyhow::Result;
use async_trait::async_trait;
use dashmap::{DashMap, DashSet};
use qexed_config::app::qexed_tcp_connect_app::TcpConnect;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage},
};
use rsa::pkcs8::EncodePublicKey;
use rsa::{RsaPrivateKey, RsaPublicKey};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpListener, sync::mpsc::UnboundedSender, time::Instant};

#[derive(Debug)]
pub struct TcpConnectManagerActor {
    config: TcpConnect,
    is_shutdown: bool,
    // 管理所有连接Actor的发送端
    _connection_senders: Arc<DashMap<SocketAddr, MessageSender<ReturnMessage<ManagerCommand>>>>,
    qexed_status_api: UnboundedSender<ReturnMessage<qexed_status::Message>>,
    qexed_player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    qexed_black_list_api: UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
    qexed_white_list_api: UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
    qexed_game_logic_api:UnboundedSender<ReturnMessage<qexed_game_logic::message::ManagerMessage>>,
    // 公私钥
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
    public_key_der: Vec<u8>,
    // 连接频率
    ip_rate_limiter: Arc<RateLimiter>,  // IP频率限制器
    ip_blacklist: Arc<DashSet<IpAddr>>, // IP黑名单（内存存储示例）
}
// 简单的IP频率限制器实现
#[derive(Debug)]
struct RateLimiter {
    attempts: DashMap<IpAddr, Vec<Instant>>,
    window: Duration,
    max_attempts: usize,
}

impl RateLimiter {
    fn new(window_secs: u64, max_attempts: usize) -> Self {
        Self {
            attempts: DashMap::new(),
            window: Duration::from_secs(window_secs),
            max_attempts,
        }
    }

    fn allow(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let window_start = now - self.window;

        let mut attempts_entry = self.attempts.entry(ip).or_insert(Vec::new());
        let attempts = attempts_entry.value_mut();

        // 清理窗口外的旧尝试
        attempts.retain(|&t| t >= window_start);

        if attempts.len() >= self.max_attempts {
            false // 超过限制
        } else {
            attempts.push(now);
            true // 允许连接
        }
    }

    // 清理长时间不活动的IP记录
    fn cleanup_old_entries(&self) {
        let now = Instant::now();
        let window_start = now - self.window;

        self.attempts.retain(|_, attempts| {
            attempts.retain(|&t| t >= window_start);
            !attempts.is_empty() // 如果清空后数组为空，移除整个条目
        });
    }
}
impl TcpConnectManagerActor {
    pub async fn new(
        config: TcpConnect,
        qexed_status_api: UnboundedSender<ReturnMessage<qexed_status::Message>>,
        qexed_player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
        qexed_black_list_api: UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
        qexed_white_list_api: UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
        qexed_game_logic_api:UnboundedSender<ReturnMessage<qexed_game_logic::message::ManagerMessage>>,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let bits = 1024; // Minecraft 使用 1024 位 RSA
        let private_key: RsaPrivateKey =
            RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
        let public_key: RsaPublicKey = RsaPublicKey::from(&private_key);
        // 将公钥编码为 DER 格式
        let public_key_der: Vec<u8> = public_key
            .to_public_key_der()
            .expect("failed to encode public key")
            .to_vec();
        let ip_rate_limiter = Arc::new(RateLimiter::new(
            config.rate_limit_window_secs,           // 从配置读取
            config.rate_limit_max_attempts as usize, // 从配置读取
        ));
        let ip_rate_limiter2 = Arc::clone(&ip_rate_limiter);
        tokio::spawn(async move{
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                ip_rate_limiter2.cleanup_old_entries();
            }
        });

        let ip_blacklist = Arc::new(DashSet::new());
        Self {
            config,
            is_shutdown: false,
            _connection_senders: Arc::new(DashMap::new()),
            qexed_status_api,
            qexed_player_list_api,
            qexed_black_list_api,
            qexed_white_list_api,
            qexed_game_logic_api,
            private_key,
            public_key,
            public_key_der,
            ip_rate_limiter,
            ip_blacklist,
        }
    }
}

#[async_trait]
impl TaskManageEvent<SocketAddr, ReturnMessage<ManagerCommand>, ReturnMessage<TaskCommand>> for TcpConnectManagerActor {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerCommand>>,
        task_map: &DashMap<SocketAddr, MessageSender<ReturnMessage<TaskCommand>>>,
        mut data: ReturnMessage<ManagerCommand>,
    ) -> Result<bool> {
        match data.data {
            ManagerCommand::Start => {
                // 启动监听循环
                let listener = TcpListener::bind(&self.config.ip).await?;
                let private_key = self.private_key.clone();
                let public_key = self.public_key.clone();
                let public_key_der = self.public_key_der.clone();
                let task_map_clone = task_map.clone();
                let api_clone = api.clone();
                let network_compression_threshold =
                    self.config.network_compression_threshold.clone();
                let online_mode = self.config.online_mode.clone();
                let status_timeout_secs = self.config.status_timeout_secs.clone();
                tokio::spawn(async move {
                    let api_clone = api_clone.clone();
                    let private_key = private_key.clone();
                    let public_key = public_key.clone();
                    let public_key_der = public_key_der.clone();
                    while let Ok((stream, addr)) = listener.accept().await {
                        let actor = TcpConnectActor::new(
                            stream,
                            addr,
                            network_compression_threshold,
                            online_mode,
                            private_key.clone(),
                            public_key.clone(),
                            public_key_der.clone(),
                            status_timeout_secs.clone(),
                        );
                        let (task, task_send) =
                            qexed_task::task::task::Task::new(api_clone.clone(), actor);
                        let is_true = task.run().await;
                        if is_true.is_err() {
                            return Err(is_true.err());
                        }
                        ReturnMessage::build(TaskCommand::Start)
                            .get(&task_send)
                            .await?;
                        task_map_clone.insert(addr, task_send);
                        // 为新连接创建ConnectionActor
                        // let message = ReturnMessage::build(ManagerCommand::NewConnection(stream, addr));
                        // let _ = api_clone.send(message);
                    }
                    Ok(())
                });
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            }
            ManagerCommand::ConnClose(addr) => {
                task_map.remove(&addr);
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            }
            ManagerCommand::GetStatusPackageBytes(ref mut value) => {
                // 客户端请求查询服务器状态,这里进行转发处理
                if let Some(respon) = ReturnMessage::build(qexed_status::Message::default())
                    .get(&self.qexed_status_api)
                    .await?
                    .data
                {
                    *value = Some(respon)
                }

                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            }
            ManagerCommand::Shutdown(ref why) => {
                self.is_shutdown = true;
                // task_map: &DashMap<SocketAddr, MessageSender<ReturnMessage<ManagerCommand>>>,
                // 安全地遍历 task_map（DashMap 的 .iter() 会获取读锁）
                for entry in task_map.iter() {
                    let sender = entry.value().clone(); // MessageSender 通常实现了 Clone
                    ReturnMessage::build(TaskCommand::Shutdown(why.clone()))
                        .get(&sender)
                        .await?;
                }
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            }
            ManagerCommand::CheckPlayeIsInList(uuid, ref mut ishave) => {
                // 客户端请求查询服务器状态,这里进行转发处理
                if let qexed_player_list::Message::CheckPlayeIsInList(_uuid, is_have) =
                    ReturnMessage::build(qexed_player_list::Message::CheckPlayeIsInList(uuid, true))
                        .get(&self.qexed_player_list_api)
                        .await?
                {
                    *ishave = is_have
                }

                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            }
            ManagerCommand::LoginCheck(uuid, addr, ref mut is_can_login, ref mut reject_reason) => {
                // 默认不可登录
                *is_can_login = false;
                // 启用代理后,我们检查ip地址将毫无意义
                // 除非从BC或者代理端向后传递ip才行
                // 我们暂时没写到BC部分
                if !self.config.proxy {
                    // 验证层 1: 提取并验证IP地址
                    let ip = match addr {
                        Some(addr) => addr,
                        None => {
                            *reject_reason = Some("无法获取客户端IP地址".to_string());
                            // 发送响应
                            if let Some(send) = data.get_return_send().await? {
                                let _ = send.send(data.data);
                            };
                            return Ok(self.is_shutdown);
                        }
                    };

                    // 验证层 2: 检查IP黑名单
                    if self.ip_blacklist.contains(&ip) {
                        *reject_reason = Some("您的IP地址已被禁止访问本服务器".to_string());
                        // 发送响应
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        };
                        return Ok(self.is_shutdown);
                    }

                    // 验证层 3: 检查IP频率限制
                    if !self.ip_rate_limiter.allow(ip) {
                        *reject_reason = Some("连接过于频繁，请等待一段时间后再试".to_string());
                        // 发送响应
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        };
                        return Ok(self.is_shutdown);
                    }
                }
                // 验证层 4: 黑名单检查
                let qexed_blacklist::Message::CheckPlayerBan(_uuid, ban_text) =
                    ReturnMessage::build(qexed_blacklist::Message::CheckPlayerBan(uuid, None))
                        .get(&self.qexed_black_list_api)
                        .await?;
                {
                    // log::debug!("正在检测UUID:{}",_uuid);
                    // log::debug!("封禁文本:{:?}",ban_text);
                    if let Some(ban_text) = ban_text {
                        *reject_reason = Some(ban_text);
                        // 发送响应
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        };
                        return Ok(self.is_shutdown);
                    }
                }
                // 验证层 5: 白名单检查
                // 请注意:Qexed 的白名单无法让你绕过反作弊,他仅仅只能限制进服
                let qexed_whitelist::Message::CheckPlayerCanJoinServer(_uuid, ban_text) =
                    ReturnMessage::build(qexed_whitelist::Message::CheckPlayerCanJoinServer(uuid, None))
                        .get(&self.qexed_white_list_api)
                        .await?;
                {
                    // log::debug!("正在检测UUID:{}",_uuid);
                    // log::debug!("封禁文本:{:?}",ban_text);
                    if let Some(ban_text) = ban_text {
                        *reject_reason = Some(ban_text);
                        // 发送响应
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        };
                        return Ok(self.is_shutdown);
                    }
                }
                
                // // 验证层 4: 可选的IP白名单检查（如果启用）
                // if self.config.enable_ip_whitelist {
                //     // 这里可以查询IP白名单服务或本地列表
                //     let is_in_whitelist = self.check_ip_whitelist(ip).await;
                //     if !is_in_whitelist {
                //         *reject_reason = Some("您的IP地址未在允许列表中".to_string());
                //         return self.send_response(data).await;
                //     }
                // }

                // 所有IP检查通过
                *is_can_login = true;
                *reject_reason = Some("IP验证通过".to_string()); // 或设置为None

                // 发送响应
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };

                return Ok(self.is_shutdown);
            },
            ManagerCommand::GetLogicApi(message) => {
                data.data = crate::messages::ManagerCommand::GetLogicApi(ReturnMessage::build(message).get(&self.qexed_game_logic_api).await?);
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                };
            },
        }
        Ok(self.is_shutdown)
    }
}
