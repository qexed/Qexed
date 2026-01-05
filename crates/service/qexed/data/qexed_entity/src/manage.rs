use dashmap::DashMap;
use qexed_task::{event::task_manage::TaskManageEvent, message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{message::{ManagerCommand, SpiltIDCommand, TaskCommand}, spilt_id_task::SplitIDActor};


pub struct ManagerActor{
    config: qexed_config::app::qexed_entity::EntityConfig,
    split_idactor:Option<UnboundedSender<SpiltIDCommand>>,
}
impl ManagerActor {
    pub fn new(config: qexed_config::app::qexed_entity::EntityConfig)->Self{
        Self { config,split_idactor:None}
    }
    
}

#[async_trait::async_trait]
impl TaskManageEvent<uuid::Uuid, ReturnMessage<ManagerCommand>, TaskCommand>
    for ManagerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerCommand>>,
        task_map: &DashMap<uuid::Uuid, MessageSender<TaskCommand>>,
        mut data: ReturnMessage<ManagerCommand>,
    ) -> anyhow::Result<bool> {
        let send = match data.get_return_send().await? {
            Some(v) => v,
            None => {
                log::error!("非法实体管理命令");
                return Ok(false);
            }
        };
        match data.data {
            ManagerCommand::Start => {
                let (st,ss) = qexed_task::task::task::Task::new(api.clone(), SplitIDActor::new());
                st.run().await?;
                let _ = ss.send(SpiltIDCommand::Start);
                self.split_idactor = Some(ss);
                let _ = send.send(data.data);
            },
            ManagerCommand::SpiltIDTaskClose => {
                drop(self.split_idactor.take());// 丢弃了
                let _ = send.send(data.data);
            }
            ManagerCommand::Register(uuid, unbounded_sender) => {
                if let Some(sp_api) = &self.split_idactor{
                    // 请注意:实体的UUID唯一
                    unbounded_sender.send(TaskCommand::Start)?;

                    // 分配实体ID
                    let (s,r) = oneshot::channel();
                    sp_api.send(SpiltIDCommand::New(s))?;
                    unbounded_sender.send(TaskCommand::AssignID(r.await?))?;
                    task_map.insert(uuid, unbounded_sender);
                    let _ = send.send(ManagerCommand::RegisterFinish);
                } else {
                    log::error!("接口调用错误,未创建id分配器,服务崩溃");
                    return Ok(true);
                }
            },
            ManagerCommand::GetEntity(uuid, ref mut unbounded_sender) => {
                if let Some(send) = task_map.get(&uuid){
                    *unbounded_sender = Some(send.clone());
                }
                let _ = send.send(data.data);
            },
            ManagerCommand::RegisterFinish=>{
                // 这不是给管理端看的。
            }
            ManagerCommand::TaskClose(uuid,entity_id) => {
                task_map.remove(&uuid);
                if entity_id!=-1{
                    if let Some(sp_api) = &self.split_idactor{
                        sp_api.send(SpiltIDCommand::Recycled(entity_id))?;
                    }
                }
                let _ = send.send(data.data);
            }
        }
        Ok(false)
    }
}