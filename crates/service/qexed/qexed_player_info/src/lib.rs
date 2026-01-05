use async_trait::async_trait;
use dashmap::DashMap;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::{login::success::Success, play::player_info::PlayerActions};
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage},
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Debug)]
pub enum Message {
    PlayerJoin(
        uuid::Uuid,
        qexed_nbt::Tag,
        Success,
        UnboundedSender<bytes::Bytes>,
    ),
    PlayerLeft(uuid::Uuid),
    AutoUpdata,
}

#[derive(Debug)]
pub struct Task {
    pub player_map: DashMap<uuid::Uuid, UnboundedSender<bytes::Bytes>>,
    pub player_success_map: DashMap<uuid::Uuid, Success>,
    pub player_chat_map:
        DashMap<uuid::Uuid, Option<qexed_protocol::to_client::play::player_info::InitializeChat>>,
    pub player_ping: DashMap<uuid::Uuid, u64>,
    pub player_display_name: DashMap<uuid::Uuid, qexed_nbt::Tag>,
    pub player_list_priority: DashMap<uuid::Uuid, i32>,
    pub new_player: Vec<Uuid>,
    pub remove_player: Vec<Uuid>,
    pub update_task: Option<tokio::task::JoinHandle<()>>,
}
impl Task {
    pub fn new() -> Self {
        Self {
            player_map: Default::default(),
            player_success_map: Default::default(),
            player_chat_map: Default::default(),
            player_ping: Default::default(),
            player_display_name: Default::default(),
            player_list_priority: Default::default(),
            new_player: Default::default(),
            remove_player: Default::default(),
            update_task: None,
        }
    }
}

#[async_trait]
impl TaskEasyEvent<Message> for Task {
    async fn event(
        &mut self,
        api: &MessageSender<Message>,
        data: Message,
    ) -> anyhow::Result<bool> {
        match data {
            Message::PlayerJoin(uuid, name, success, player_api) => {
                self.player_map.insert(uuid, player_api);
                self.player_success_map.insert(uuid, success);
                self.player_chat_map.insert(uuid, None);
                self.player_ping.insert(uuid, 33);
                self.player_display_name.insert(uuid, name);
                self.player_list_priority.insert(uuid, 1000);
                self.new_player.push(uuid);
                if let None = self.update_task {
                    let api2 = api.clone();
                    self.update_task = Some(tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        let _ = api2.send(Message::AutoUpdata);
                    }))
                }
                return Ok(false);
            }
            Message::PlayerLeft(uuid) => {
                self.player_map.remove(&uuid);
                self.player_success_map.remove(&uuid);
                self.player_chat_map.remove(&uuid);
                self.player_ping.remove(&uuid);
                self.player_display_name.remove(&uuid);
                self.player_list_priority.remove(&uuid);
                self.remove_player.push(uuid);
                if let None = self.update_task {
                    let api2 = api.clone();
                    self.update_task = Some(tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        let _ = api2.send(Message::AutoUpdata);
                    }))
                }
                return Ok(false);
            }
            Message::AutoUpdata => {
                // 1. 构建玩家移除数据包
                let remove_pk = PacketSend::build_send_packet(
                    qexed_protocol::to_client::play::player_info_remove::PlayerInfoRemove {
                        uuids: self.remove_player.clone(),
                    },
                )
                .await?;

                // 2. 准备新玩家数据 - 构建 PlayerInfo 数据包
                let mut players_to_update = Vec::new();
                let action_mask: u8 = 0b1111_1111;

                // 遍历新玩家列表，构建每个玩家的更新数据
                for uuid in &self.new_player {
                    if let Some(success_data) = self.player_success_map.get(uuid) {
                        let mut player_actions = Vec::new();

                        // 添加玩家基础信息 (AddPlayer 动作)

                        player_actions.push(PlayerActions::AddPlayer(success_data.username.clone(),success_data.properties.clone()));
                        player_actions.push(PlayerActions::InitializeChat(None));
                        player_actions.push(PlayerActions::UpdateGameMode(VarInt(0)));// 暂时写死生存
                        player_actions.push(PlayerActions::UpdateListed(true));
                        if let Some(ping) = self.player_ping.get(uuid) {
                            player_actions.push(PlayerActions::UpdateLatency(VarInt(*ping as i32)));
                        }
                        if let Some(display_name) = self.player_display_name.get(uuid) {
                            player_actions
                                .push(PlayerActions::UpdateDisplayName(Some(display_name.clone())));
                        }
                        if let Some(priority) = self.player_list_priority.get(uuid) {
                            player_actions
                                .push(PlayerActions::UpdateListPriority(VarInt(*priority)));
                        }
                        player_actions.push(PlayerActions::UpdateHat(true));



                        // 构建玩家条目
                        players_to_update.push(qexed_protocol::to_client::play::player_info::Players {
                            uuid: *uuid,
                            data: player_actions,
                        });
                    }
                }

                // 3. 构建玩家信息更新数据包
                let update_list_pk = PacketSend::build_send_packet(
                    qexed_protocol::to_client::play::player_info::PlayerInfo {
                        action: action_mask,
                        players: players_to_update,
                    },
                )
                .await?;
                // 4. 向所有在线玩家发送更新数据包
                for sender in &self.player_map {
                    // 先发送玩家移除通知
                    let _ = sender.send(remove_pk.clone());

                    // 只有有新玩家需要更新时才发送 PlayerInfo 数据包
                    if action_mask != 0 {
                        let _ = sender.send(update_list_pk.clone());
                    }
                }

                // 5. 清理已处理的列表
                self.new_player.clear();
                self.remove_player.clear();
                self.update_task=None;
                Ok(false)
            }
        }
    }
}

pub async fn run() -> anyhow::Result<UnboundedSender<Message>> {
    let task_data = Task::new();
    let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
    task.run().await?;
    log::info!("[服务] 玩家信息 已启用");
    Ok(task_send)
}
