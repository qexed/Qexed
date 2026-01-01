use async_trait::async_trait;
use bytes::Bytes;
use qexed_config::app::qexed_heartbeat::HeartbeatConfig;
use qexed_tcp_connect::PacketSend;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{
    mpsc::{Receiver, Sender, UnboundedSender, channel},
    Mutex,
};
use uuid::Uuid;

use crate::message::{HeartbeatPhase, InternalMessage, ManagerCommand, StateChange, TaskCommand};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};

// 心跳状态
#[derive(Debug)]
struct HeartbeatState {
    config: HeartbeatConfig,
    is_running: bool,
    is_paused: bool,
    phase: HeartbeatPhase,
    
    // 心跳相关
    last_heartbeat_time: Option<Instant>,
    last_heartbeat_id: i64,
    pending_heartbeats: std::collections::HashMap<i64, (Instant, HeartbeatPhase)>,
    consecutive_misses: u32,
}

impl HeartbeatState {
    fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            is_running: false,
            is_paused: false,
            phase: HeartbeatPhase::Configuration,
            last_heartbeat_time: None,
            last_heartbeat_id: 0,
            pending_heartbeats: std::collections::HashMap::new(),
            consecutive_misses: 0,
        }
    }
    
    fn start_heartbeat(&mut self) -> Option<(i64, HeartbeatPhase)> {
        if !self.is_running || self.is_paused {
            return None;
        }
        
        self.last_heartbeat_id += 1;
        let heartbeat_id = self.last_heartbeat_id;
        let current_phase = self.phase;
        
        self.last_heartbeat_time = Some(Instant::now());
        self.pending_heartbeats.insert(heartbeat_id, (Instant::now(), current_phase));
        
        // 清理旧心跳（优化：只在必要时清理）
        if self.pending_heartbeats.len() > 10 {
            self.cleanup_old_heartbeats();
        }
        
        Some((heartbeat_id, current_phase))
    }
    
    fn handle_heartbeat_response(&mut self, heartbeat_id: i64) -> bool {
        self.pending_heartbeats.remove(&heartbeat_id).map(|_| {
            self.last_heartbeat_time = Some(Instant::now());
            self.consecutive_misses = 0;
            true
        }).unwrap_or(false)
    }
    
    fn check_timeouts(&mut self) -> Vec<(i64, HeartbeatPhase)> {
        let now = Instant::now();
        let timeout = Duration::from_secs(self.config.timeout_seconds as u64);
        
        // 收集超时的心跳 - 修复引用模式问题
        let timed_out: Vec<_> = self.pending_heartbeats
            .iter()
            .filter(|&(_, &(sent_time, _))| now.duration_since(sent_time) > timeout)
            .map(|(&id, &(_, phase))| (id, phase))
            .collect();
        
        // 移除超时的心跳
        for (id, _) in &timed_out {
            self.pending_heartbeats.remove(id);
        }
        
        timed_out
    }
    
    fn cleanup_old_heartbeats(&mut self) {
        let now = Instant::now();
        let cleanup_threshold = Duration::from_secs(self.config.timeout_seconds as u64 * 2);
        
        self.pending_heartbeats
            .retain(|_, &mut (sent_time, _)| now.duration_since(sent_time) <= cleanup_threshold);
    }
    
    fn set_state(&mut self, running: bool, paused: bool) {
        self.is_running = running;
        self.is_paused = paused;
        
        if !running || paused {
            self.pending_heartbeats.clear();
        }
    }
    
    fn set_phase(&mut self, phase: HeartbeatPhase) {
        if self.phase != phase {
            log::debug!("Heartbeat phase changed from {:?} to {:?}", self.phase, phase);
            self.phase = phase;
        }
    }
    
    fn should_send_heartbeat(&self) -> bool {
        if !self.is_running || self.is_paused {
            return false;
        }
        
        match self.last_heartbeat_time {
            Some(last_heartbeat) => {
                let elapsed = Instant::now().duration_since(last_heartbeat);
                let interval = Duration::from_secs(self.config.interval_seconds as u64);
                elapsed >= interval
            }
            None => true,
        }
    }
    
    fn has_pending_heartbeats(&self) -> bool {
        !self.pending_heartbeats.is_empty()
    }
    
    fn get_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.config.timeout_seconds as u64)
    }
    
    fn increment_miss_count(&mut self) {
        self.consecutive_misses += 1;
    }
    
    fn reset_miss_count(&mut self) {
        self.consecutive_misses = 0;
    }
    
    fn has_exceeded_max_misses(&self) -> bool {
        self.consecutive_misses >= self.config.max_consecutive_misses
    }
}

#[derive(Debug)]
pub struct HeartbeatTask {
    config: HeartbeatConfig,
    player_uuid: Uuid,
    packet_send: UnboundedSender<Bytes>,
    internal_sender: Sender<InternalMessage>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl HeartbeatTask {
    pub fn new(
        config: HeartbeatConfig,
        player_uuid: Uuid,
        packet_send: UnboundedSender<Bytes>,
    ) -> Self {
        let (internal_sender, _) = channel(100);
        
        Self {
            player_uuid,
            packet_send,
            internal_sender,
            task_handle: None,
            config,
        }
    }
    
    async fn send_internal(&self, msg: InternalMessage) -> Result<(), anyhow::Error> {
        self.internal_sender
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send internal message: {e}"))
    }
    
    async fn send_heartbeat(
        packet_send: &UnboundedSender<Bytes>,
        heartbeat_id: i64,
        player_uuid: Uuid,
        phase: HeartbeatPhase,
    ) -> anyhow::Result<()> {
        let heartbeat_data = match phase {
            HeartbeatPhase::Configuration | HeartbeatPhase::Play => {
                // 两个阶段目前都使用相同的KeepAlive包结构
                PacketSend::build_send_packet::<qexed_protocol::to_client::play::keep_alive::KeepAlive>(
                    qexed_protocol::to_client::play::keep_alive::KeepAlive { 
                        keep_alive_id: heartbeat_id,
                    },
                ).await?
            }
        };
        
        packet_send
            .send(heartbeat_data)
            .map_err(|e| anyhow::anyhow!("Failed to send heartbeat packet: {e}"))
    }
    
    async fn notify_heartbeat_status(
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        player_uuid: Uuid,
        status: crate::message::HeartbeatStatus,
    ) {
        let _ = ReturnMessage::build(ManagerCommand::HeartbeatStatus(player_uuid, status))
            .post(manage_api)
            .await;
    }
    
    async fn handle_timeout(
        api: &MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        player_uuid: Uuid,
        phase: HeartbeatPhase,
    ) {
        log::warn!("Heartbeat timeout for player {player_uuid} in {phase:?} phase");
        
        // 通知管理器心跳超时
        Self::notify_heartbeat_status(
            manage_api,
            player_uuid,
            crate::message::HeartbeatStatus::Timeout(player_uuid, phase),
        ).await;
        
        // 关闭连接
        let _ = UnReturnMessage::build(TaskCommand::Close).post(api).await;
    }
    
    fn start_background_task(
        &mut self,
        mut receiver: Receiver<InternalMessage>,
        api: MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: MessageSender<ReturnMessage<ManagerCommand>>,
    ) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let player_uuid = self.player_uuid;
        let packet_send = self.packet_send.clone();
        
        tokio::spawn(async move {
            let mut state = HeartbeatState::new(config);
            let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(1));
            
            loop {
                tokio::select! {
                    biased; // 优先处理内部消息
                    
                    // 处理内部消息
                    Some(msg) = receiver.recv() => {
                        if let Err(e) = Self::handle_internal_message(
                            &mut state, msg, &packet_send, player_uuid, &api, &manage_api
                        ).await {
                            log::error!("Failed to handle internal message: {e}");
                            break;
                        }
                    }
                    
                    // 心跳定时检查
                    _ = heartbeat_interval.tick() => {
                        if let Err(e) = Self::handle_heartbeat_tick(
                            &mut state, &packet_send, player_uuid, &api, &manage_api
                        ).await {
                            log::error!("Failed to handle heartbeat tick: {e}");
                            break;
                        }
                    }
                }
            }
        })
    }
    
    async fn handle_internal_message(
        state: &mut HeartbeatState,
        msg: InternalMessage,
        packet_send: &UnboundedSender<Bytes>,
        player_uuid: Uuid,
        api: &MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
    ) -> anyhow::Result<()> {
        match msg {
            InternalMessage::SendHeartbeat => {
                if let Some((heartbeat_id, phase)) = state.start_heartbeat() {
                    Self::send_heartbeat(packet_send, heartbeat_id, player_uuid, phase).await?;
                }
            }
            
            InternalMessage::HeartbeatReceived(heartbeat_id) => {
                if state.handle_heartbeat_response(heartbeat_id) {
                    if let Some(timestamp) = state.last_heartbeat_time {
                        let timestamp_secs = timestamp.elapsed().as_secs();
                        Self::notify_heartbeat_status(
                            manage_api,
                            player_uuid,
                            crate::message::HeartbeatStatus::Alive(timestamp_secs, state.phase),
                        ).await;
                    }
                } else {
                    log::warn!("Received unknown heartbeat ID {heartbeat_id} from player {player_uuid}");
                }
            }
            
            InternalMessage::StateChange(change) => {
                match change {
                    StateChange::Running(running) => state.set_state(running, state.is_paused),
                    StateChange::Paused(paused) => state.set_state(state.is_running, paused),
                }
            }
            
            InternalMessage::PhaseChange(phase) => {
                state.set_phase(phase);
                log::debug!("Player {player_uuid} heartbeat phase changed to {phase:?}");
            }
            
            InternalMessage::Shutdown => {
                return Err(anyhow::anyhow!("Shutdown requested"));
            }
        }
        
        Ok(())
    }
    
    async fn handle_heartbeat_tick(
        state: &mut HeartbeatState,
        packet_send: &UnboundedSender<Bytes>,
        player_uuid: Uuid,
        api: &MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
    ) -> anyhow::Result<()> {
        // 检查是否需要发送心跳
        if state.should_send_heartbeat() {
            if let Some((heartbeat_id, phase)) = state.start_heartbeat() {
                Self::send_heartbeat(packet_send, heartbeat_id, player_uuid, phase).await?;
            }
        }
        
        // 检查是否有心跳超时
        if state.has_pending_heartbeats() {
            let timed_out = state.check_timeouts();
            for (heartbeat_id, phase) in timed_out {
                log::debug!("Heartbeat {heartbeat_id} timed out for player {player_uuid} in {phase:?} phase");
                state.increment_miss_count();
                
                if state.has_exceeded_max_misses() {
                    log::warn!(
                        "Player {player_uuid} heartbeat timed out {} times in {phase:?} phase, disconnecting", 
                        state.consecutive_misses
                    );
                    Self::handle_timeout(api, manage_api, player_uuid, phase).await;
                    return Err(anyhow::anyhow!("Max consecutive misses exceeded"));
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl TaskEvent<UnReturnMessage<TaskCommand>, ReturnMessage<ManagerCommand>> for HeartbeatTask {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: UnReturnMessage<TaskCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskCommand::Start => {
                if self.task_handle.is_none() {
                    let (sender, receiver) = channel(100);
                    self.internal_sender = sender;
                    
                    let handle = self.start_background_task(
                        receiver,
                        api.clone(),
                        manage_api.clone(),
                    );
                    self.task_handle = Some(handle);
                    
                    self.send_internal(InternalMessage::StateChange(StateChange::Running(true)))
                        .await?;
                }
            }
            
            TaskCommand::Pause => {
                self.send_internal(InternalMessage::StateChange(StateChange::Paused(true)))
                    .await?;
            }
            
            TaskCommand::Stop => {
                self.send_internal(InternalMessage::StateChange(StateChange::Running(false)))
                    .await?;
            }
            
            TaskCommand::Heartbeat(heartbeat_id) => {
                self.send_internal(InternalMessage::HeartbeatReceived(heartbeat_id))
                    .await?;
            }
            
            TaskCommand::Part(is_play_part) => {
                let phase = if is_play_part {
                    HeartbeatPhase::Play
                } else {
                    HeartbeatPhase::Configuration
                };
                
                self.send_internal(InternalMessage::PhaseChange(phase))
                    .await?;
                
                log::info!("Player {} heartbeat phase changed to {:?}", self.player_uuid, phase);
            }
            
            TaskCommand::Close => {
                // 发送关闭信号
                let _ = self.send_internal(InternalMessage::Shutdown).await;
                
                // 等待后台任务结束
                if let Some(handle) = self.task_handle.take() {
                    handle.abort();
                }
                
                // 获取当前阶段（可以添加一个方法来获取）
                let phase = HeartbeatPhase::Configuration; // 可以从状态中获取
                
                // 通知管理器连接关闭
                Self::notify_heartbeat_status(
                    manage_api,
                    self.player_uuid,
                    crate::message::HeartbeatStatus::Disconnected(self.player_uuid, phase),
                ).await;
                
                log::info!("Closing heartbeat task for player {} in {:?} phase", self.player_uuid, phase);
                return Ok(true);
            }
        }
        
        Ok(false)
    }
}