use async_trait::async_trait;
use dashmap::DashMap;
use qexed_config::app::qexed_game_logic::GameLogicConfig;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage},
};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::{
    message::{ManagerMessage, NewPlayerConnectError, TaskMessage},
    task::GameLogicActor,
};

#[derive(Debug)]
pub struct GameLogicManagerActor {
    config: GameLogicConfig,
    registry_data: Vec<qexed_protocol::to_client::configuration::registry_data::RegistryData>,
    tags: qexed_protocol::to_client::configuration::tags::Tags,
    qexed_ping_api:UnboundedSender<ReturnMessage<qexed_ping::message::ManagerCommand>>,
    qexed_heartbeat_api:UnboundedSender<ReturnMessage<qexed_heartbeat::message::ManagerCommand>>,
    qexed_packet_split_api:UnboundedSender<ReturnMessage<qexed_packet_split::message::ManagerMessage>>,
    qexed_chat_api:UnboundedSender<ReturnMessage<qexed_chat::message::ManagerMessage>>,
    qexed_command_api:UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    qexed_player_list_api:UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    qexed_chunk_api:UnboundedSender<UnReturnMessage<qexed_chunk::message::global::GlobalCommand>>,
    qexed_title_api:UnboundedSender<ReturnMessage<qexed_title::message::ManagerMessage>>,
}
impl GameLogicManagerActor {
    pub fn new(
        config: GameLogicConfig,
        registry_data: Vec<qexed_protocol::to_client::configuration::registry_data::RegistryData>,
        tags: qexed_protocol::to_client::configuration::tags::Tags,
        qexed_ping_api:UnboundedSender<ReturnMessage<qexed_ping::message::ManagerCommand>>,
        qexed_heartbeat_api:UnboundedSender<ReturnMessage<qexed_heartbeat::message::ManagerCommand>>,
        qexed_packet_split_api:UnboundedSender<ReturnMessage<qexed_packet_split::message::ManagerMessage>>,
        qexed_chat_api:UnboundedSender<ReturnMessage<qexed_chat::message::ManagerMessage>>,
        qexed_command_api:UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
        qexed_player_list_api:UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
        qexed_chunk_api:UnboundedSender<UnReturnMessage<qexed_chunk::message::global::GlobalCommand>>,
        qexed_title_api:UnboundedSender<ReturnMessage<qexed_title::message::ManagerMessage>>,
    ) -> Self {
        Self {
            config,
            registry_data,
            tags,
            qexed_ping_api,
            qexed_heartbeat_api,
            qexed_packet_split_api,
            qexed_chat_api,
            qexed_command_api,
            qexed_player_list_api,
            qexed_chunk_api,
            qexed_title_api,
        }
    }

}
#[async_trait]
impl TaskManageEvent<Uuid, ReturnMessage<ManagerMessage>, ReturnMessage<TaskMessage>>
    for GameLogicManagerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerMessage>>,
        task_map: &DashMap<Uuid, MessageSender<ReturnMessage<TaskMessage>>>,
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
                if let Some(player_api) = task_map.get(&uuid) {
                    // let _ = ReturnMessage::build(TaskMessage::Close).post(&player_api).await;
                    // task_map.remove(&uuid);
                    *err = Some(NewPlayerConnectError::PlayerNotAway.into());
                    let _ = send.send(data.data);
                    return Ok(false);
                }
                let (task, task_sand) = crate::task::Task::new(api.clone(), GameLogicActor::new(uuid));
                task.run().await?;
                task_map.insert(uuid, task_sand.clone());
                *task_api = Some(task_sand);
                *is_true = true;
                let _ = send.send(data.data);
                return Ok(false);
            }
            ManagerMessage::Registry(ref mut d1,ref mut d2) =>{
                *d1 = Some(self.registry_data.clone());
                *d2 = Some(self.tags.clone());
                let _ = send.send(data.data);
                return Ok(false);
            }
            ManagerMessage::PlayerClose(uuid) => {
                task_map.remove(&uuid);
                let _ = send.send(data.data);
                return Ok(false);
            }
            ManagerMessage::ConnectClose(uuid) =>{
                if let Some(task_api) = task_map.get(&uuid) {
                    ReturnMessage::build(TaskMessage::Close).get(&task_api).await?;
                } 
                task_map.remove(&uuid);
                let _ = send.send(data.data);
                return Ok(false);

            }
            ManagerMessage::GetPlayerPing(mut ping) => {

                let ping = match ping.take(){
                    Some(ping) => ping,
                    None => return Ok(false)
                };

                let data = crate::message::ManagerMessage::GetPlayerPing(Some(ReturnMessage::build(ping).get(&self.qexed_ping_api).await?));
                let _ = send.send(data);
                return Ok(false);
            }
            ManagerMessage::GetPlayerHeartbeat(mut heartbeat) => {

                let ping = match heartbeat.take(){
                    Some(ping) => ping,
                    None => return Ok(false)
                };

                let data = crate::message::ManagerMessage::GetPlayerHeartbeat(Some(ReturnMessage::build(ping).get(&self.qexed_heartbeat_api).await?));
                let _ = send.send(data);
                return Ok(false);
            }
            ManagerMessage::GetPlayerPacketSplit(mut packet_split) => {

                let ping = match packet_split.take(){
                    Some(ping) => ping,
                    None => return Ok(false)
                };

                let data = crate::message::ManagerMessage::GetPlayerPacketSplit(Some(ReturnMessage::build(ping).get(&self.qexed_packet_split_api).await?));
                let _ = send.send(data);
                return Ok(false);
            }
            ManagerMessage::GetPlayerChat(mut chat_message) =>{
                let chat = match chat_message.take(){
                    Some(ping) => ping,
                    None => return Ok(false)
                };

                let data = crate::message::ManagerMessage::GetPlayerChat(Some(ReturnMessage::build(chat).get(&self.qexed_chat_api).await?));
                let _ = send.send(data);
                return Ok(false);
            }
            ManagerMessage::GetTitle(mut chat_message) =>{
                let chat = match chat_message.take(){
                    Some(ping) => ping,
                    None => return Ok(false)
                };

                let data = crate::message::ManagerMessage::GetTitle(Some(ReturnMessage::build(chat).get(&self.qexed_title_api).await?));
                let _ = send.send(data);
                return Ok(false);
            }
            ManagerMessage::GetCommand(mut chat_message) =>{
                let chat = match chat_message.take(){
                    Some(ping) => ping,
                    None => return Ok(false)
                };

                let data = crate::message::ManagerMessage::GetCommand(Some(ReturnMessage::build(chat).get(&self.qexed_command_api).await?));
                let _ = send.send(data);
                return Ok(false);
            }
            ManagerMessage::GetPlayerListApi(ref mut api)=>{
                *api = Some(self.qexed_player_list_api.clone());
                let _ = send.send(data.data);
                return Ok(false);
            }            
        }
    }
}
