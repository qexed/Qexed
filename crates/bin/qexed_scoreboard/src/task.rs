use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use qexed_nbt::Tag;
use qexed_packet::{codec::vec, net_types::VarInt};
use qexed_protocol::to_client::play::{set_objective::NumberFormat, system_chat::SystemChat};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::message::{ManagerMessage, TaskMessage};

#[derive(Debug)]
pub struct ScoreBoardActor {
    uuid: Uuid,
    name: String,
    packet_write: Option<UnboundedSender<Bytes>>,
    config: qexed_config::app::qexed_scoreboard::ScoreBoardConfig,
    update:Vec<(i32,String)>,
    players:i32,
}
impl ScoreBoardActor {
    pub fn new(uuid: Uuid, config: qexed_config::app::qexed_scoreboard::ScoreBoardConfig) -> Self {
        Self {
            uuid,
            name: "".to_string(),
            packet_write: None,
            config: config,
            update:vec![],
            players:-1,
        }
    }
    // async fn config
    fn build_text(&self,message: String) -> Tag {
        // 1. 创建文本组件的 Compound
        let message = message.replace("{player}", &self.name).replace("{online}", &format!("{}",&self.players));
        let mut chat_component = HashMap::new();
        // Minecraft 文本组件的基础格式：{"text": "实际内容"}
        chat_component.insert(
            "text".to_string(),
            Tag::String(message.into()), // 使用 `into()` 转为 Arc<str>
        );

        // 2. 可选：添加样式（例如颜色）
        // chat_component.insert("color".to_string(), Tag::String("red".into()));

        // 3. 将 HashMap 包装为 Tag::Compound
        Tag::Compound(Arc::new(chat_component))
    }

}
#[async_trait]
impl TaskEvent<UnReturnMessage<TaskMessage>, ReturnMessage<ManagerMessage>> for ScoreBoardActor {
    async fn event(
        &mut self,
        _api: &MessageSender<UnReturnMessage<TaskMessage>>,
        manage_api: &MessageSender<ReturnMessage<ManagerMessage>>,
        mut data: UnReturnMessage<TaskMessage>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskMessage::Start(name, mut unbounded_sender) => {
                // 玩家进入了服务器
                self.name = name;
                self.packet_write = unbounded_sender.take();
                let packet_write = match self.packet_write.clone() {
                    Some(p) => p,
                    None => {
                        return Ok(false);
                    }
                };

                packet_write.send(
                    PacketSend::build_send_packet(
                        // 构建创建计分板数据包
                        qexed_protocol::to_client::play::set_objective::SetObjective {
                            name: "display_qexed_scoreboard".to_string(),
                            action: 0,
                            display_text: self.build_text(self.config.name.clone()),
                            r#type: qexed_packet::net_types::VarInt(0),
                            number_format: Some(NumberFormat::Blank),
                        },
                    )
                    .await?,
                )?;
                // 循环添加每一行
                let line_len = self.config.line.len();
                for (i, line_text) in self.config.line.iter().enumerate() {
                    // 1. 为每一行生成一个唯一的“实体名”
                    // Minecraft协议要求分数必须关联一个实体（玩家、虚拟实体等）
                    // 常用技巧：使用颜色代码生成“伪玩家名”，既显示文本又保证唯一
                    let score_name = format!("§r{}", line_text); // 例如 "§a§a第一行"

                    // 2. 构建并发送 SetScore 包
                    let score_packet = qexed_protocol::to_client::play::set_score::SetScore {
                        entity_name: score_name.clone(),
                        scoreboard_name: "display_qexed_scoreboard".to_string(),
                        value: qexed_packet::net_types::VarInt((line_len - i - 1) as i32),
                        display_text:Some(self.build_text(score_name.clone())),
                        number_format:Some(NumberFormat::Blank),
                    };
                    if score_name.contains("{online}") {
                        self.update.push(((line_len - i - 1) as i32,score_name));
                    };
                    packet_write.send(
                        PacketSend::build_send_packet(
                            score_packet
                        )
                        .await?,
                    )?;
                }
                packet_write.send(
                    PacketSend::build_send_packet(
                        // 显示计分板
                        qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective {
                            position: qexed_packet::net_types::VarInt(1),
                            name:"display_qexed_scoreboard".to_string(), // 要显示的目标名
                        }
                    ).await?
                )?;
                
                return Ok(false);
            }
            TaskMessage::UpdatePlayers(players) => {
                self.players = players;
                let packet_write = match self.packet_write.clone() {
                    Some(p) => p,
                    None => {
                        return Ok(false);
                    }
                };
                for (i, line_text) in self.update.iter() {
                    let score_packet = qexed_protocol::to_client::play::set_score::SetScore {
                        entity_name: line_text.clone(),
                        scoreboard_name: "display_qexed_scoreboard".to_string(),
                        value: qexed_packet::net_types::VarInt(*i),
                        display_text:Some(self.build_text(line_text.clone())),
                        number_format:Some(NumberFormat::Blank),
                    };
                    packet_write.send(
                        PacketSend::build_send_packet(
                            score_packet
                        )
                        .await?,
                    )?;
                };
                return Ok(false);
            }
            TaskMessage::Close => {
                // 向父级发送关闭消息
                ReturnMessage::build(ManagerMessage::PlayerClose(self.uuid))
                    .get(manage_api)
                    .await?;
                return Ok(true);
            }
        }
    }
}
