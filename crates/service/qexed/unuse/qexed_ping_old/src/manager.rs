use async_trait::async_trait;
use dashmap::DashMap;
use qexed_config::app::qexed_ping::PingConfig;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
    task::task::Task,
};

use crate::{
    message::{ManagerCommand, NewPlayerConnectError, TaskCommand},
    task::PingTask,
};

#[derive(Debug)]
pub struct PingManagerActor {
    config: PingConfig,
}
impl PingManagerActor {
    pub fn new(config: PingConfig) -> Self {
        Self { config: config }
    }
}
#[async_trait]
impl TaskManageEvent<uuid::Uuid, ReturnMessage<ManagerCommand>, UnReturnMessage<TaskCommand>>
    for PingManagerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerCommand>>,
        task_map: &DashMap<uuid::Uuid, MessageSender<UnReturnMessage<TaskCommand>>>,
        mut data: ReturnMessage<ManagerCommand>,
    ) -> anyhow::Result<bool> {
        let send = match data.get_return_send().await? {
            Some(send) => send,
            None => return Ok(false),
        };
        match data.data {
            ManagerCommand::NewPlayerConnect(
                ref mut uuid,
                _is_true,
                _err,
                _task_api,
                mut packet_send,
            ) => {

                if task_map.contains_key(&uuid) {
                    let _ = send.send(ManagerCommand::NewPlayerConnect(
                        uuid.clone(),
                        false,
                        Some(NewPlayerConnectError::PlayerNotAway.into()),
                        None,
                        None,
                    ));
                    return Ok(false);
                }
                let packet_send: tokio::sync::mpsc::UnboundedSender<bytes::Bytes> =
                    match packet_send.take() {
                        Some(pk) => pk,
                        None => {
                            let _ = send.send(ManagerCommand::NewPlayerConnect(
                                *uuid, false, None, None, None,
                            ));

                            return Ok(false);
                        }
                    };
                let t = PingTask::new(self.config.clone(), uuid.clone(), packet_send);
                let (task, task_sand) = Task::new(api.clone(), t);
                task.run().await?;
                task_map.insert(*uuid, task_sand.clone());

                let _ = send.send(ManagerCommand::NewPlayerConnect(
                    *uuid,
                    true,
                    None,
                    Some(task_sand),
                    None,
                ));

                return Ok(false);
            }
            ManagerCommand::PlayerClose(uuid) => {
                task_map.remove(&uuid);

                let _ = send.send(data.data);

                return Ok(false);
            }
        }
    }
}
