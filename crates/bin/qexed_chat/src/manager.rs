use async_trait::async_trait;
use dashmap::DashMap;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
    task::task::Task,
};
use uuid::Uuid;

use crate::{
    message::{ManagerMessage, NewPlayerConnectError, TaskMessage},
    task::ChatActor,
};

#[derive(Debug)]
pub struct ChatManagerActor {
    config: qexed_config::app::qexed_chat::ChatConfig,
}
impl ChatManagerActor {
    pub fn new(config: qexed_config::app::qexed_chat::ChatConfig) -> Self {
        Self { config: config }
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
                let _ = send.send(ManagerMessage::BroadCastEvent(_uuid,system_chat));
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
