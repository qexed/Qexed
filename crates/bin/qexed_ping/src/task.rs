use async_trait::async_trait;
use bytes::Bytes;
use qexed_config::app::qexed_ping::PingConfig;

use crate::message::{ManagerCommand, Part, TaskCommand};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use qexed_tcp_connect::PacketSend;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedSender, channel};
use tokio::sync::oneshot;

// 内部消息类型
#[derive(Debug)]
enum InternalMessage {
    SendPing,
    PongReceived(i32, Instant),
    TimeoutCheck,
    AwaitBarrier(oneshot::Sender<bool>),
    StateChange(StateChange),
}

#[derive(Debug)]
enum StateChange {
    Running(bool),
    Paused(bool),
    PartChanged(Part),
}

// Ping 请求状态
#[derive(Debug)]
struct PingRequest {
    id: i32,
    sent_time: Instant,
    retry_count: u32,
}

// 无锁的 Ping 状态
#[derive(Debug)]
struct PingState {
    config: PingConfig,
    current_part: Part,
    is_running: bool,
    is_paused: bool,

    // 正在进行的 Ping 请求
    pending_pings: HashMap<i32, PingRequest>,

    // 最后成功的时间
    last_success: Option<Instant>,

    // 重试计数器
    consecutive_failures: u32,

    // 下一个 Ping ID
    next_ping_id: i32,

    // 最后测量的延迟
    last_latency: Option<Duration>,

    // 等待屏障队列
    await_barriers: VecDeque<oneshot::Sender<bool>>,

    // 超时设置
    timeout: Duration,
}

impl PingState {
    fn new(config: PingConfig) -> Self {
        let timeout = Duration::from_secs(config.interval as u64 * 2);

        Self {
            config,
            current_part: Part::Configuration,
            is_running: false,
            is_paused: false,
            pending_pings: HashMap::new(),
            last_success: None,
            consecutive_failures: 0,
            next_ping_id: 0,
            last_latency: None,
            await_barriers: VecDeque::new(),
            timeout,
        }
    }

    fn start_ping(&mut self) -> Result<Option<(i32, Instant)>, anyhow::Error> {
        if !self.is_running || self.is_paused {
            return Ok(None);
        }

        // 检查是否超过最大重试次数
        if self.consecutive_failures >= self.config.max_retries as u32 {
            return Err(anyhow::anyhow!("Max retries exceeded"));
        }

        // 检查延迟限制
        if self.config.enable_latency_limit {
            if let Some(last_latency) = self.last_latency {
                if last_latency.as_millis() > self.config.latency_limit_ms as u128 {
                    return Err(anyhow::anyhow!("Latency limit exceeded"));
                }
            }
        }

        // 生成新的 Ping ID
        self.next_ping_id += 1;
        let ping_id = self.next_ping_id;
        let sent_time = Instant::now();

        // 记录请求
        self.pending_pings.insert(
            ping_id,
            PingRequest {
                id: ping_id,
                sent_time,
                retry_count: 0,
            },
        );

        // 清理过期的请求
        self.cleanup_expired_requests();

        Ok(Some((ping_id, sent_time)))
    }

    fn handle_pong(
        &mut self,
        pong_id: i32,
        received_time: Instant,
    ) -> Result<Duration, anyhow::Error> {
        // 查找对应的 Ping 请求
        if let Some(request) = self.pending_pings.remove(&pong_id) {
            let latency = received_time.duration_since(request.sent_time);

            // 更新状态
            self.last_success = Some(received_time);
            self.last_latency = Some(latency);
            self.consecutive_failures = 0;

            // 清理过期的请求
            self.cleanup_expired_requests();

            // 响应等待屏障
            self.resolve_await_barriers();

            return Ok(latency);
        }

        // 无效的 Pong ID
        Err(anyhow::anyhow!("Invalid pong ID: {}", pong_id))
    }

    fn check_timeouts(&mut self) -> Vec<(i32, u32)> {
        let now = Instant::now();
        let mut timeouts = Vec::new();
        let mut to_remove = Vec::new();

        for (ping_id, request) in &mut self.pending_pings {
            if now.duration_since(request.sent_time) > self.timeout {
                request.retry_count += 1;
                timeouts.push((*ping_id, request.retry_count));

                if request.retry_count > 1 {
                    // 超过一次重试就移除
                    to_remove.push(*ping_id);
                }
            }
        }

        // 移除多次超时的请求
        for ping_id in to_remove {
            self.pending_pings.remove(&ping_id);
        }

        timeouts
    }

    fn has_timeout_requests(&self) -> bool {
        let now = Instant::now();
        self.pending_pings
            .values()
            .any(|req| now.duration_since(req.sent_time) > self.timeout)
    }

    fn get_timeout_requests(&self) -> Vec<i32> {
        let now = Instant::now();
        self.pending_pings
            .iter()
            .filter_map(|(id, req)| {
                if now.duration_since(req.sent_time) > self.timeout {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_next_ping_time(&self) -> Option<Duration> {
        if !self.is_running || self.is_paused || self.has_timeout_requests() {
            return None;
        }

        let interval = Duration::from_secs(self.config.interval as u64);

        // 如果还有未完成的请求，等待它们超时
        if let Some(oldest_request) = self.pending_pings.values().min_by_key(|req| req.sent_time) {
            let time_since_oldest = Instant::now().duration_since(oldest_request.sent_time);
            if time_since_oldest < self.timeout {
                return Some(self.timeout - time_since_oldest);
            }
        }

        // 计算下一次发送时间
        if let Some(last_success) = self.last_success {
            let time_since_last = Instant::now().duration_since(last_success);
            if time_since_last < interval {
                return Some(interval - time_since_last);
            }
        }

        Some(Duration::from_secs(0))
    }

    fn add_await_barrier(&mut self, sender: oneshot::Sender<bool>) {
        self.await_barriers.push_back(sender);

        // 如果没有未完成的请求，立即解决
        if self.pending_pings.is_empty() {
            self.resolve_await_barriers();
        }
    }

    fn resolve_await_barriers(&mut self) {
        while let Some(sender) = self.await_barriers.pop_front() {
            let _ = sender.send(self.pending_pings.is_empty());
        }
    }

    fn cleanup_expired_requests(&mut self) {
        let now = Instant::now();
        let cleanup_threshold = self.timeout * 10;

        self.pending_pings
            .retain(|_, req| now.duration_since(req.sent_time) <= cleanup_threshold);
    }

    fn set_running(&mut self, running: bool) {
        self.is_running = running;
        if !running {
            self.pending_pings.clear();
            self.resolve_await_barriers();
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
        if paused {
            self.resolve_await_barriers();
        }
    }

    fn set_part(&mut self, part: Part) {
        self.current_part = part;
        self.pending_pings.clear();
        self.consecutive_failures = 0;
        self.resolve_await_barriers();
    }
}

#[derive(Debug)]
pub struct PingTask {
    player_uuid: uuid::Uuid,
    packet_send: UnboundedSender<Bytes>,

    // 内部通信通道
    internal_sender: Sender<InternalMessage>,
    state: PingState,

    // 任务句柄
    task_handle: Option<tokio::task::JoinHandle<()>>,

    // 超时检查间隔
    check_interval: Duration,
}

impl PingTask {
    pub fn new(
        config: PingConfig,
        player_uuid: uuid::Uuid,
        packet_send: UnboundedSender<Bytes>,
    ) -> Self {
        let (internal_sender, _) = channel(100);
        let state = PingState::new(config);
        let check_interval = Duration::from_millis(100); // 100ms 检查一次

        Self {
            player_uuid,
            packet_send,
            internal_sender,
            state,
            task_handle: None,
            check_interval,
        }
    }

    // 发送内部消息
    async fn send_internal(&self, msg: InternalMessage) -> Result<(), anyhow::Error> {
        self.internal_sender
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send internal message: {}", e))
    }

    // 启动后台任务并返回句柄
    fn start_background_task(
        mut self,
        mut receiver: Receiver<InternalMessage>,
        api: MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: MessageSender<ReturnMessage<ManagerCommand>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ping_timer =
                tokio::time::interval(Duration::from_secs(self.state.config.interval as u64));
            let mut check_timer = tokio::time::interval(self.check_interval);

            // 等待第一次 tick
            ping_timer.tick().await;
            check_timer.tick().await;

            loop {
                tokio::select! {
                    // 处理内部消息
                    Some(msg) = receiver.recv() => {
                        match msg {
                            InternalMessage::SendPing => {
                                if let Err(e) = self.send_ping_impl().await {
                                    log::error!("Failed to send ping: {}", e);
                                    break;
                                }
                            }
                            InternalMessage::PongReceived(pong_id, timestamp) => {
                                if let Err(e) = self.handle_pong_impl(pong_id, timestamp).await {
                                    log::error!("Failed to handle pong: {}", e);
                                }
                            }
                            InternalMessage::TimeoutCheck => {
                                self.check_timeouts_impl(&api).await;
                            }
                            InternalMessage::AwaitBarrier(sender) => {
                                self.state.add_await_barrier(sender);
                            }
                            InternalMessage::StateChange(change) => {
                                match change {
                                    StateChange::Running(running) => self.state.set_running(running),
                                    StateChange::Paused(paused) => self.state.set_paused(paused),
                                    StateChange::PartChanged(part) => self.state.set_part(part),
                                }
                            }
                        }
                    }

                    // Ping 定时器
                    _ = ping_timer.tick(), if self.state.is_running && !self.state.is_paused => {
                        if let Err(e) = self.send_ping_impl().await {
                            log::error!("Failed to send ping: {}", e);
                            break;
                        }
                    }

                    // 超时检查定时器
                    _ = check_timer.tick(), if self.state.is_running => {
                        self.check_timeouts_impl(&api).await;
                    }

                    // 检查是否需要退出
                    else => {
                        if !self.state.is_running {
                            break;
                        }
                    }
                }
            }

            // 清理资源
            self.state.pending_pings.clear();
            self.state.resolve_await_barriers();
        })
    }

    async fn send_ping_impl(&mut self) -> Result<(), anyhow::Error> {
        match self.state.start_ping() {
            Ok(Some((ping_id, _))) => {
                // 构建并发送 Ping 包
                let ping_data = self.build_ping_data(ping_id).await?;
                self.packet_send
                    .send(ping_data)
                    .map_err(|e| anyhow::anyhow!("Failed to send ping packet: {}", e))?;

                log::debug!("Sent ping {} to player {}", ping_id, self.player_uuid);
                Ok(())
            }
            Ok(None) => Ok(()), // 未运行或暂停状态
            Err(e) => {
                log::warn!("Cannot send ping: {}", e);
                // 触发断开连接
                Err(e)
            }
        }
    }

    async fn handle_pong_impl(
        &mut self,
        pong_id: i32,
        timestamp: Instant,
    ) -> Result<(), anyhow::Error> {
        match self.state.handle_pong(pong_id, timestamp) {
            Ok(latency) => {
                log::debug!(
                    "Player {} pong received, latency: {:?}",
                    self.player_uuid,
                    latency
                );

                // 检查延迟限制
                if self.state.config.enable_latency_limit {
                    let latency_ms = latency.as_millis() as i32;
                    if latency_ms > self.state.config.latency_limit_ms {
                        log::warn!("Player {} high latency: {}ms", self.player_uuid, latency_ms);
                        // 可以发送警告消息
                    }
                }

                Ok(())
            }
            Err(e) => {
                log::warn!("Invalid pong from player {}: {}", self.player_uuid, e);
                // 记录无效响应
                self.state.consecutive_failures += 1;

                if self.state.consecutive_failures >= 3 {
                    return Err(anyhow::anyhow!("Too many invalid pong responses"));
                }

                Ok(())
            }
        }
    }

    async fn check_timeouts_impl(&mut self, api: &MessageSender<UnReturnMessage<TaskCommand>>) {
        let timeouts = self.state.check_timeouts();

        for (ping_id, retry_count) in timeouts {
            log::debug!(
                "Ping {} timeout for player {}, retry: {}",
                ping_id,
                self.player_uuid,
                retry_count
            );

            if retry_count >= 2 {
                // 第二次重试就认为失败
                self.state.consecutive_failures += 1;

                if self.state.consecutive_failures >= self.state.config.max_retries as u32 {
                    log::warn!(
                        "Player {} exceeded max retries, disconnecting",
                        self.player_uuid
                    );
                    // 通知管理器断开连接
                    let _ = UnReturnMessage::build(TaskCommand::Close).post(api).await;
                    break;
                }
            }
        }
    }

    async fn build_ping_data(&self, ping_id: i32) -> anyhow::Result<Bytes> {
        match self.state.current_part {
            Part::Configuration => PacketSend::build_send_packet::<
                qexed_protocol::to_client::configuration::ping::Ping,
            >(
                qexed_protocol::to_client::configuration::ping::Ping { id: ping_id },
            )
            .await,
            Part::Play => {
                // Play 阶段的 Ping 处理
                PacketSend::build_send_packet::<qexed_protocol::to_client::play::ping::Ping>(
                    qexed_protocol::to_client::play::ping::Ping { id: ping_id },
                )
                .await
            }
        }
    }
}

#[async_trait]
impl TaskEvent<UnReturnMessage<TaskCommand>, ReturnMessage<ManagerCommand>> for PingTask {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: UnReturnMessage<TaskCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskCommand::Start => {
                // 启动后台任务
                if self.task_handle.is_none() {
                    let (sender, receiver) = channel(100);
                    self.internal_sender = sender;

                    let config = self.state.config.clone();
                    let player_uuid = self.player_uuid;
                    let packet_send = self.packet_send.clone();
                    let check_interval = self.check_interval;

                    let handle = Self {
                        player_uuid,
                        packet_send,
                        internal_sender: self.internal_sender.clone(),
                        state: PingState::new(config),
                        task_handle: None,
                        check_interval,
                    }
                    .start_background_task(
                        receiver,
                        api.clone(),
                        manage_api.clone(),
                    );

                    self.task_handle = Some(handle);
                }

                self.state.set_running(true);
                log::debug!("Ping started for player {}", self.player_uuid);
            }

            TaskCommand::Pause => {
                self.state.set_paused(true);
                log::debug!("Ping paused for player {}", self.player_uuid);
            }

            TaskCommand::Stop => {
                self.state.set_running(false);
                if let Some(handle) = self.task_handle.take() {
                    handle.abort();
                }
                log::debug!("Ping stopped for player {}", self.player_uuid);
            }

            TaskCommand::UpdatePart(part) => {
                let part_clone = part.clone();
                self.send_internal(InternalMessage::StateChange(StateChange::PartChanged(part)))
                    .await?;
                log::debug!(
                    "Player {} switched to {:?} part",
                    self.player_uuid,
                    part_clone
                );
            }

            TaskCommand::Await(sender) => {
                self.send_internal(InternalMessage::AwaitBarrier(sender))
                    .await?;
            }

            TaskCommand::Pong(timestamp) => {
                let now = Instant::now();
                self.send_internal(InternalMessage::PongReceived(timestamp, now))
                    .await?;
            }

            TaskCommand::Close => {
                self.state.set_running(false);

                // 等待后台任务结束
                if let Some(handle) = self.task_handle.take() {
                    handle.abort();
                }

                // 通知管理器
                ReturnMessage::build(ManagerCommand::PlayerClose(self.player_uuid))
                    .get(manage_api)
                    .await?;

                log::info!("Closing ping task for player {}", self.player_uuid);
                return Ok(true);
            }
        }

        Ok(false)
    }
}
