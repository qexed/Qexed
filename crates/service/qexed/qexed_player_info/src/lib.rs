use std::collections::HashMap;

use async_trait::async_trait;
use dashmap::DashMap;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::{login::success::Success, play::player_info::PlayerActions};
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage},
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

#[derive(Debug)]
pub enum Message {
    PlayerJoin(
        uuid::Uuid,
        qexed_nbt::Tag,
        Success,
        UnboundedSender<bytes::Bytes>,
        oneshot::Sender<()>
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
    pub new_player: HashMap<Uuid,oneshot::Sender<()>>,
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
    async fn event(&mut self, api: &MessageSender<Message>, data: Message) -> anyhow::Result<bool> {
        match data {
            Message::PlayerJoin(uuid, name, success, player_api,send) => {
                self.player_map.insert(uuid, player_api.clone());
                self.player_success_map.insert(uuid, success);
                self.player_chat_map.insert(uuid, None);
                self.player_ping.insert(uuid, 33);
                self.player_display_name.insert(uuid, name);
                self.player_list_priority.insert(uuid, 1000);
                self.new_player.insert(uuid,send);
                // 5. 构建旧玩家信息数据包（给新玩家看）
                let mut old_players_to_update = Vec::new();

                // 遍历所有已存在的玩家（不包括新加入的）
                for i in &self.player_success_map {
                    let uuid = i.key();
                    let success_data = i.value();

                    let mut player_actions = Vec::new();
                    player_actions.push(PlayerActions::AddPlayer(
                        success_data.username.clone(),
                        success_data.properties.clone(),
                    ));
                    player_actions.push(PlayerActions::InitializeChat(None));
                    player_actions.push(PlayerActions::UpdateGameMode(VarInt(0)));
                    player_actions.push(PlayerActions::UpdateListed(true));

                    if let Some(ping) = self.player_ping.get(uuid) {
                        player_actions.push(PlayerActions::UpdateLatency(VarInt(*ping as i32)));
                    }
                    if let Some(display_name) = self.player_display_name.get(uuid) {
                        player_actions
                            .push(PlayerActions::UpdateDisplayName(Some(display_name.clone())));
                    }
                    if let Some(priority) = self.player_list_priority.get(uuid) {
                        player_actions.push(PlayerActions::UpdateListPriority(VarInt(*priority)));
                    }
                    player_actions.push(PlayerActions::UpdateHat(true));

                    old_players_to_update.push(
                        qexed_protocol::to_client::play::player_info::Players {
                            uuid: *uuid,
                            data: player_actions,
                        },
                    );
                }
                
                // 6. 向所有新玩家发送：旧玩家列表
                let old_player_list_pk = PacketSend::build_send_packet(
                    qexed_protocol::to_client::play::player_info::PlayerInfo {
                        action: 0b1111_1111,
                        players: old_players_to_update,
                    },
                )
                .await?;
                let _ = player_api.send(old_player_list_pk.clone());
                
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
                // 1. 构建玩家移除数据包（如果需要）
                let remove_pk =
                    if !self.remove_player.is_empty() {
                        Some(PacketSend::build_send_packet(
                        qexed_protocol::to_client::play::player_info_remove::PlayerInfoRemove {
                            uuids: self.remove_player.clone(),
                        },
                    ).await?)
                    } else {
                        None
                    };

                // 2. 构建新玩家信息数据包（给老玩家看）
                let mut new_players_to_update = Vec::new();
                let action_mask: u8 = 0b1111_1111;

                for (uuid,_) in &self.new_player {
                    if let Some(success_data) = self.player_success_map.get(uuid) {
                        let mut player_actions = Vec::new();

                        player_actions.push(PlayerActions::AddPlayer(
                            success_data.username.clone(),
                            success_data.properties.clone(),
                        ));
                        player_actions.push(PlayerActions::InitializeChat(None));
                        player_actions.push(PlayerActions::UpdateGameMode(VarInt(0)));
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

                        new_players_to_update.push(
                            qexed_protocol::to_client::play::player_info::Players {
                                uuid: *uuid,
                                data: player_actions,
                            },
                        );
                    }
                }

                // 3. 构建新玩家信息更新数据包
                let new_player_update_pk = if !new_players_to_update.is_empty() {
                    Some(
                        PacketSend::build_send_packet(
                            qexed_protocol::to_client::play::player_info::PlayerInfo {
                                action: action_mask,
                                players: new_players_to_update,
                            },
                        )
                        .await?,
                    )
                } else {
                    None
                };

                // 4. 向所有老玩家发送：移除通知 + 新玩家加入通知
                for i in &self.player_map {
                    let uuid = i.key();

                    // 发送新玩家加入通知（如果有）
                    if let Some(ref new_player_packet) = new_player_update_pk {
                        let _ = i.value().send(new_player_packet.clone());
                    }
                    // 关键修复：先判断，后发送
                    if self.new_player.contains_key(uuid) {
                        continue; // 新玩家不接收这些通知
                    }
                    // 发送移除通知（如果有）
                    if let Some(ref remove_packet) = remove_pk {
                        let _ = i.value().send(remove_packet.clone());
                    }


                }
                    let uuids_to_process: Vec<Uuid> = self.new_player.keys().cloned().collect();
                    for uuid in uuids_to_process {
                        // 发送信号（优雅处理 RecvError）
                        if let Some(signal_sender) = self.new_player.remove(&uuid) {
                            if signal_sender.send(()).is_err() {
                                // 正常情况：接收端可能已关闭
                            }
                        }
                    }
                // 7. 清理已处理的列表
                self.new_player.clear();
                self.remove_player.clear();
                self.update_task = None;
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
