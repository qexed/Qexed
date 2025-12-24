use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use dashmap::DashMap;
use qexed_nbt::Tag;
use qexed_protocol::to_client::play::system_chat::{self, SystemChat};
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
    task::task::Task,
};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::{
    message::{ManagerMessage, NewPlayerConnectError, TaskMessage},
    task::ChatActor,
};

#[derive(Debug)]
pub struct ChatManagerActor {
    config: qexed_config::app::qexed_chat::ChatConfig,
    player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
}
impl ChatManagerActor {
    pub fn new(
        config: qexed_config::app::qexed_chat::ChatConfig,
        player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    ) -> Self {
        Self {
            config: config,
            player_list_api: player_list_api,
        }
    }
}
#[async_trait]
impl TaskManageEvent<Uuid, ReturnMessage<ManagerMessage>, UnReturnMessage<TaskMessage>>
    for ChatManagerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerMessage>>,
        task_map: &DashMap<Uuid, MessageSender<UnReturnMessage<TaskMessage>>>,
        mut data: ReturnMessage<ManagerMessage>,
    ) -> anyhow::Result<bool> {
        let send = match data.get_return_send().await? {
            Some(send) => send,
            None => return Ok(false),
        };
        match data.data {
            ManagerMessage::NewPlayerConnect(
                uuid,
                ref mut is_true,
                ref mut err,
                ref mut task_api,
            ) => {
                if task_map.contains_key(&uuid) {
                    *err = Some(NewPlayerConnectError::PlayerNotAway.into());
                    let _ = send.send(data.data);
                    return Ok(false);
                }
                let (task, task_sand) =
                    Task::new(api.clone(), ChatActor::new(uuid, self.config.clone()));
                task.run().await?;
                task_map.insert(uuid, task_sand.clone());
                *task_api = Some(task_sand);
                *is_true = true;
                let _ = send.send(data.data);
                return Ok(false);
            }
            ManagerMessage::BroadCastEvent(_uuid, system_chat) => {
                for task in task_map {
                    let _ = task.send(UnReturnMessage::build(TaskMessage::SendMessage(
                        system_chat.clone(),
                    )));
                }
                let _ = send.send(ManagerMessage::BroadCastEvent(_uuid, system_chat));
                return Ok(false);
            }
            ManagerMessage::Command(ref cmd) => {
                let args = cmd.parse_args();
                let help_args: Vec<String> = args.into_iter().skip(1).collect();
                if help_args.len() < 2 {
                    cmd.send_chat_message("§c用法: /tell <玩家> <消息>").await?;
                    let _ = send.send(data.data);
                    return Ok(false);
                }
                let player_name = match &cmd.player_name {
                    Some(name) => name,
                    None => "系统",
                };
                let target_player = &help_args[0];
                let message = &help_args[1..].join(" ");
                if let qexed_player_list::Message::GetPlayerIsOnline {
                    name: _,
                    is_true,
                    player_uuid,
                } = ReturnMessage::build(qexed_player_list::Message::GetPlayerIsOnline {
                    name: target_player.clone(),
                    is_true: false,
                    player_uuid: uuid::Uuid::nil(),
                })
                .get(&self.player_list_api)
                .await?
                {
                    if is_true == false {
                        cmd.send_chat_message(&format!("§c玩家 {} 不在线", target_player))
                            .await?;
                        let _ = send.send(data.data);
                        return Ok(false);
                    }
                    if let Some(other_player_api) = task_map.get(&player_uuid) {
                        // build_chat_packet(message:String)
                        other_player_api.send(UnReturnMessage::build(
                            TaskMessage::SendMessage(build_chat_packet(format!(
                                "§5[私聊] §r[{}§r] §r{}",
                                player_name,
                                message.clone()
                            ))),
                        ))?;
                        cmd.send_chat_message(&format!(
                            "§5[私聊] §r[{}§r->{}§r] §r{}",
                            player_name,
                            target_player.clone(),
                            message
                        ))
                        .await?;
                        let _ = send.send(data.data);
                        return Ok(false);
                    }

                    cmd.send_chat_message(&format!("§c玩家 {} 不在线", target_player))
                        .await?;
                    let _ = send.send(data.data);
                    return Ok(false);
                } else {
                    cmd.send_chat_message(&format!("§c玩家 {} 不在线", target_player))
                        .await?;
                    let _ = send.send(data.data);
                    return Ok(false);
                }
            }
            ManagerMessage::CommandMe(ref cmd) => {
                let args = cmd.parse_args();
                let help_args: Vec<String> = args.into_iter().skip(1).collect();
                if help_args.len() < 1 {
                    cmd.send_chat_message("§c用法: /me <消息>").await?;
                    let _ = send.send(data.data);
                    return Ok(false);
                }
                let player_name = match &cmd.player_name {
                    Some(name) => name,
                    None => "系统",
                };
                let message = &help_args.join(" ");
                let system_chat = build_chat_packet(format!(
                                "* §r{}§r §r{}",
                                player_name,
                                message.clone()
                            ));
                for task in task_map {
                    let _ = task.send(UnReturnMessage::build(TaskMessage::SendMessage(
                        system_chat.clone(),
                    )));
                }
                let _ = send.send(data.data);
                return Ok(false);
            }
            ManagerMessage::CommandSay(ref cmd) => {
                let args = cmd.parse_args();
                let help_args: Vec<String> = args.into_iter().skip(1).collect();
                if help_args.len() < 1 {
                    cmd.send_chat_message("§c用法: /say <消息>").await?;
                    let _ = send.send(data.data);
                    return Ok(false);
                }
                let player_name = match &cmd.player_name {
                    Some(name) => name,
                    None => "系统",
                };
                let message = &help_args.join(" ");
                let system_chat = build_chat_packet(format!(
                                "[§r{}§r] §r{}",
                                player_name,
                                message.clone()
                            ));
                for task in task_map {
                    let _ = task.send(UnReturnMessage::build(TaskMessage::SendMessage(
                        system_chat.clone(),
                    )));
                }
                let _ = send.send(data.data);
                return Ok(false);
            }
            
            ManagerMessage::PlayerClose(uuid) => {
                task_map.remove(&uuid);
                let _ = send.send(data.data);

                return Ok(false);
            }
            ManagerMessage::ConnectClose(uuid) => {
                if let Some(task_api) = task_map.get(&uuid) {
                    UnReturnMessage::build(TaskMessage::Close)
                        .post(&task_api)
                        .await?;
                }
                task_map.remove(&uuid);
                let _ = send.send(data.data);
                return Ok(false);
            }
        }
    }
}
// async fn config
fn build_chat_packet(message: String) -> SystemChat {
    // 1. 创建文本组件的 Compound
    let mut chat_component = HashMap::new();
    // Minecraft 文本组件的基础格式：{"text": "实际内容"}
    chat_component.insert(
        "text".to_string(),
        Tag::String(message.into()), // 使用 `into()` 转为 Arc<str>
    );

    // 2. 可选：添加样式（例如颜色）
    // chat_component.insert("color".to_string(), Tag::String("red".into()));

    // 3. 将 HashMap 包装为 Tag::Compound
    let content_nbt = Tag::Compound(Arc::new(chat_component));

    // 4. 构建 SystemChat（overlay = false 表示显示在普通聊天框）
    SystemChat {
        content: content_nbt,
        overlay: false,
    }
}
