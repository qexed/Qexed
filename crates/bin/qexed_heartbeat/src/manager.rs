use async_trait::async_trait;
use dashmap::DashMap;
use qexed_config::app::qexed_heartbeat::HeartbeatConfig;
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
    task::HeartbeatTask,
};

#[derive(Debug)]
pub struct HeartbeatManagerActor {
    config: HeartbeatConfig,
}

impl HeartbeatManagerActor {
    pub fn new(config: HeartbeatConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl TaskManageEvent<uuid::Uuid, ReturnMessage<ManagerCommand>, UnReturnMessage<TaskCommand>>
    for HeartbeatManagerActor
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
                // 检查玩家是否已存在
                if task_map.contains_key(&uuid) {
                    let _ = send.send(ManagerCommand::NewPlayerConnect(
                        uuid.clone(),
                        false,
                        Some(NewPlayerConnectError::PlayerAlreadyExists.into()),
                        None,
                        None,
                    ));
                    return Ok(false);
                }
                
                // 获取数据包发送通道
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
                
                // 创建心跳任务
                let t = HeartbeatTask::new(self.config.clone(), uuid.clone(), packet_send);
                let (task, task_sand) = Task::new(api.clone(), t);
                task.run().await?;
                
                // 保存任务通道
                task_map.insert(*uuid, task_sand.clone());
                
                // 返回成功
                let _ = send.send(ManagerCommand::NewPlayerConnect(
                    *uuid,
                    true,
                    None,
                    Some(task_sand),
                    None,
                ));
                
                Ok(false)
            }
            
            ManagerCommand::PlayerClose(uuid) => {
                // 移除心跳任务
                task_map.remove(&uuid);
                let _ = send.send(data.data);
                Ok(false)
            }
            
            ManagerCommand::HeartbeatStatus(uuid, status) => {
                match status {
                    crate::message::HeartbeatStatus::Alive(_, heartbeat_phase) => {},
                    crate::message::HeartbeatStatus::Timeout(uuid, heartbeat_phase) => {
                        task_map.remove(&uuid);
                    },
                    crate::message::HeartbeatStatus::Disconnected(uuid, heartbeat_phase) => {
                        task_map.remove(&uuid);
                    },
                }
                let _ = send.send(ManagerCommand::HeartbeatStatus(uuid, status));
                Ok(false)
            }
        }
    }
}