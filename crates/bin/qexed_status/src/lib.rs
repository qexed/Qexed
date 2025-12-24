use async_trait::async_trait;
use qexed_config::app::{qexed_status::StatusConfig};
use qexed_task::{
    event::task::{TaskEasyEvent},
    message::{MessageSender, MessageType, return_message::ReturnMessage},
};
use rand::seq::SliceRandom;
use serde_json::json;
use bytes::Bytes;
use tokio::{
    sync::mpsc::UnboundedSender, time::Instant,
};

#[derive(Debug, Clone,Default)]
pub struct Message {
    pub data: Option<Bytes>,
}

#[derive(Debug)]
pub struct Task {
    /// 缓存的状态数据
    cache: Option<Bytes>,
    /// 缓存时间（秒，-1为不缓存）
    cache_time: i32,
    /// 服务器描述随机内容
    motd: Vec<String>,
    /// 上次缓存时间
    last_cache_time: Instant,
    /// 服务器logo
    favicon:String,
    /// 微服务: 玩家数 api
    player_length_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
}

impl Task {
    pub fn new(
        config: StatusConfig,
        player_length_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    ) -> Self {
        Self {
            cache_time: config.cache,
            motd: config.motd,
            last_cache_time: Instant::now(),
            cache: None,
            player_length_api,
            favicon:config.favicon,
        }
    }

    /// 检查缓存是否有效
    fn is_cache_valid(&self) -> bool {
        if self.cache_time < 0 {
            return false; // 不启用缓存
        }

        if let Some(_) = self.cache {
            let elapsed = self.last_cache_time.elapsed();
            elapsed.as_secs() < self.cache_time as u64
        } else {
            false // 没有缓存数据
        }
    }

    /// 获取玩家数（从玩家数API微服务）
    async fn get_player_count(&self) -> anyhow::Result<(i32, i32)> {
        // 构建请求消息
        let response = ReturnMessage::build(qexed_player_list::Message::LoadData(0,0))
        .get(&self.player_length_api)
        .await?;
        if let qexed_player_list::Message::LoadData(player,max_player) = response{
            return Ok((player,max_player))
        };
        Err(anyhow::anyhow!("Unexpected response type from player length API"))
        
    }

    /// 构建完整的服务器状态JSON
    async fn build_status_json(&mut self) -> anyhow::Result<Bytes> {
        // 获取玩家数
        let (current_players, max_players) = self.get_player_count().await?;

        // 随机选择一个MOTD
        let motd = self
            .motd
            .choose(&mut rand::thread_rng())
            .cloned()
            .unwrap_or_else(|| "A Minecraft Server".to_string());

        // 构建状态JSON
        let status = json!({
            "version": {
                "name": format!("Qexed {}",qexed_config::MC_VERSION),
                "protocol": qexed_config::PROTOCOL_VERSION
            },
            "players": {
                "max": max_players,
                "online": current_players,
                "sample": [] // 可以后续添加在线玩家示例
            },
            "description": {
                "text": motd
            },
            "favicon": self.favicon, // 可替换为实际favicon
            "enforcesSecureChat": true,
            "previewsChat": true
        });
        
        Ok(qexed_tcp_connect::PacketSend::build_send_packet(qexed_protocol::to_client::status::server_info::ServerInfo{
            response:status
        }).await?)
    }

    /// 更新缓存
    async fn update_cache(&mut self) -> anyhow::Result<()> {
        let status_json = self.build_status_json().await?;
        self.cache = Some(status_json);
        self.last_cache_time = Instant::now();
        Ok(())
    }
}

#[async_trait]
impl TaskEasyEvent<ReturnMessage<Message>> for Task {
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<Message>>,
        mut data: ReturnMessage<Message>,
    ) -> anyhow::Result<bool> {
        let status_json = if self.is_cache_valid() {
            // 缓存有效，直接使用缓存
            self.cache.clone().unwrap()
        } else {
            // 缓存无效或未启用，更新缓存
            self.update_cache().await?;
            self.cache.clone().unwrap()
        };
        data.data.data = Some(status_json);
        // 如果有返回通道，发送响应
        if let Some(send) = data.get_return_send().await? {
            let _ = send.send(data.data);
        }

        Ok(false) // 不停止任务
    }
}

pub async fn run(
    config: StatusConfig,
    player_length_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
) -> anyhow::Result<UnboundedSender<ReturnMessage<Message>>> {
    // 创建任务数据
    let task_data = Task::new(config, player_length_api);

    // 创建并运行任务
    let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
    task.run().await?;
    log::info!("[服务] 服务器状态 已启用");
    Ok(task_send)
}
