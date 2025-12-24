use async_trait::async_trait;
use dashmap::DashMap;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage},
    task::task::Task,
};
use uuid::Uuid;

use crate::{
    message::{ManagerMessage, NewPlayerConnectError, TaskMessage},
    task::QexedPacketSplitActor,
};

#[derive(Debug)]
pub struct PacketSplitManagerActor {
    _config: qexed_config::app::qexed_packet_split::PacketSplitConfig,
}
impl PacketSplitManagerActor {
    pub fn new(
        config: qexed_config::app::qexed_packet_split::PacketSplitConfig,
    ) -> Self {
        Self {
            _config:config,
        }
    }
}
#[async_trait]
impl TaskManageEvent<Uuid, ReturnMessage<ManagerMessage>, ReturnMessage<TaskMessage>>
    for PacketSplitManagerActor
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
                if task_map.contains_key(&uuid) {
                    *err = Some(NewPlayerConnectError::PlayerNotAway.into());
                    let _ = send.send(data.data);
                    return Ok(false);
                }
                let (task, task_sand) = Task::new(api.clone(), QexedPacketSplitActor::new(uuid));
                task.run().await?;
                task_map.insert(uuid, task_sand.clone());
                *task_api = Some(task_sand);
                *is_true = true;
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
        }
    }
}
