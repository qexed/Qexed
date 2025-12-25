// use std::{collections::HashMap, sync::Arc};

// use async_trait::async_trait;
// use bytes::Bytes;
// use qexed_nbt::Tag;
// use qexed_protocol::to_client::{ play::system_chat::SystemChat};
// use qexed_task::{
//     event::task::TaskEvent,
//     message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage},
// };
// use qexed_tcp_connect::PacketSend;
// use tokio::sync::mpsc::UnboundedSender;
// use uuid::Uuid;

// use crate::message::{ManagerMessage, TaskMessage};

// #[derive(Debug)]
// pub struct TitleActor {
//     uuid: Uuid,
//     name: String,
//     packet_write: Option<UnboundedSender<Bytes>>,
//     config: qexed_config::app::qexed_title::TitleConfig,
// }
// impl TitleActor {
//     pub fn new(uuid: Uuid,config: qexed_config::app::qexed_title::TitleConfig,) -> Self {
//         Self {
//             uuid,
//             name: "".to_string(),
//             packet_write: None,
//             config:config,
//         }
//     }
// }
// #[async_trait]
// impl TaskEvent<UnReturnMessage<TaskMessage>, ReturnMessage<ManagerMessage>> for TitleActor {
//     async fn event(
//         &mut self,
//         _api: &MessageSender<UnReturnMessage<TaskMessage>>,
//         manage_api: &MessageSender<ReturnMessage<ManagerMessage>>,
//         mut data: UnReturnMessage<TaskMessage>,
//     ) -> anyhow::Result<bool> {
//         match data.data {
//             TaskMessage::Start(
//                 name,
//                 mut unbounded_sender,
//             ) => {
//                 // 玩家进入了服务器
//                 self.name = name;
//                 self.packet_write = unbounded_sender.take();
//                 let _packet_write = match self.packet_write.clone() {
//                     Some(p) => p,
//                     None => {
//                         return Ok(false);
//                     }
//                 };
//                 return Ok(false);
//             }
//             TaskMessage::SendTitleMessage(system_chat) => {
//                 if let Some(packet_write) = &self.packet_write{
//                     packet_write.send(PacketSend::build_send_packet(system_chat).await?)?;
//                 }
//                 return Ok(false);
//             },
//             TaskMessage::Close => {
//                 // 向父级发送关闭消息
//                 ReturnMessage::build(ManagerMessage::PlayerClose(self.uuid))
//                     .get(manage_api)
//                     .await?;
//                 return Ok(true);
//             }
//         }
//     }
// }
// // async fn config
// fn build_chat(message:String) -> SystemChat {
//     // 1. 创建文本组件的 Compound
//     let mut chat_component = HashMap::new();
//     // Minecraft 文本组件的基础格式：{"text": "实际内容"}
//     chat_component.insert(
//         "text".to_string(),
//         Tag::String(message.into()) // 使用 `into()` 转为 Arc<str>
//     );

//     // 2. 可选：添加样式（例如颜色）
//     // chat_component.insert("color".to_string(), Tag::String("red".into()));

//     // 3. 将 HashMap 包装为 Tag::Compound
//     let content_nbt = Tag::Compound(Arc::new(chat_component));

//     // 4. 构建 SystemChat（overlay = false 表示显示在普通聊天框）
//     SystemChat {
//         content: content_nbt,
//         overlay: false,
//     }
// }