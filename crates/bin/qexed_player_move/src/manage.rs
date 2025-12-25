// use std::{collections::HashMap, sync::Arc};

// use async_trait::async_trait;
// use dashmap::DashMap;
// use qexed_nbt::Tag;
// use qexed_protocol::to_client::play::{set_title_text::SetTitleText, system_chat::SystemChat};
// use qexed_task::{
//     event::task_manage::TaskManageEvent,
//     message::{
//         MessageSender, MessageType, return_message::ReturnMessage,
//         unreturn_message::UnReturnMessage,
//     },
//     task::task::Task,
// };
// use tokio::sync::mpsc::UnboundedSender;
// use uuid::Uuid;

// use crate::{
//     message::{ManagerMessage, NewPlayerConnectError, TaskMessage},
//     task::TitleActor,
// };

// #[derive(Debug)]
// pub struct TitleManagerActor {
//     config: qexed_config::app::qexed_title::TitleConfig,
//     player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
// }
// impl TitleManagerActor {
//     pub fn new(
//         config: qexed_config::app::qexed_title::TitleConfig,
//         player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
//     ) -> Self {
//         Self {
//             config: config,
//             player_list_api: player_list_api,
//         }
//     }
// }
// #[async_trait]
// impl TaskManageEvent<Uuid, ReturnMessage<ManagerMessage>, UnReturnMessage<TaskMessage>>
//     for TitleManagerActor
// {
//     async fn event(
//         &mut self,
//         api: &MessageSender<ReturnMessage<ManagerMessage>>,
//         task_map: &DashMap<Uuid, MessageSender<UnReturnMessage<TaskMessage>>>,
//         mut data: ReturnMessage<ManagerMessage>,
//     ) -> anyhow::Result<bool> {
//         let send = match data.get_return_send().await? {
//             Some(send) => send,
//             None => return Ok(false),
//         };
//         match data.data {
//             ManagerMessage::NewPlayerConnect(
//                 uuid,
//                 ref mut is_true,
//                 ref mut err,
//                 ref mut task_api,
//             ) => {
//                 if task_map.contains_key(&uuid) {
//                     *err = Some(NewPlayerConnectError::PlayerNotAway.into());
//                     let _ = send.send(data.data);
//                     return Ok(false);
//                 }
//                 let (task, task_sand) =
//                     Task::new(api.clone(), TitleActor::new(uuid, self.config.clone()));
//                 task.run().await?;
//                 task_map.insert(uuid, task_sand.clone());
//                 *task_api = Some(task_sand);
//                 *is_true = true;
//                 let _ = send.send(data.data);
//                 return Ok(false);
//             }
//             ManagerMessage::Command(ref cmd) => {
//                 // cmd.send_chat_message("§c开发中,请等待后续更新支持").await?;
//                 let args = cmd.parse_args();
//                 let help_args: Vec<String> = args.into_iter().skip(1).collect();
//                 if help_args.len() < 1 {
//                     cmd.send_chat_message("§c请使用 /help title 查看方法").await?;
//                     let _ = send.send(data.data);
//                     return Ok(false);
//                 }
//                 // match &help_args[0] {
// // 
//                 //     _ =>{
//                 //         cmd.send_chat_message("§c未知或未支持的请使用 /help title 查看方法").await?;
//                 //         let _ = send.send(data.data);
//                 //         return Ok(false);
//                 //     }
//                 //     
//                 // }
//                 let title = build_set_title_text_packet(help_args[0].clone());

//                 for task in task_map {
//                     let _ = task.send(UnReturnMessage::build(TaskMessage::SendTitleMessage(
//                         title.clone(),
//                     )));
//                 }
//                 let _ = send.send(data.data);
//                 return Ok(false);
//             }
            
//             ManagerMessage::PlayerClose(uuid) => {
//                 task_map.remove(&uuid);
//                 let _ = send.send(data.data);

//                 return Ok(false);
//             }
//             ManagerMessage::ConnectClose(uuid) => {
//                 if let Some(task_api) = task_map.get(&uuid) {
//                     UnReturnMessage::build(TaskMessage::Close)
//                         .post(&task_api)
//                         .await?;
//                 }
//                 task_map.remove(&uuid);
//                 let _ = send.send(data.data);
//                 return Ok(false);
//             }
//         }
//     }
// }
// // async fn config
// fn build_set_title_text_packet(message: String) -> SetTitleText {
//     SetTitleText {
//         text: create_text_nbt(&message),
//     }
// }
// fn create_text_nbt(text: &str) -> qexed_nbt::Tag {
//     let mut map = std::collections::HashMap::new();
//     map.insert("text".to_string(), qexed_nbt::Tag::String(text.to_string().into()));
//     qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
// }