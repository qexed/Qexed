use dashmap::DashMap;
use qexed_task::{event::task_manage::TaskManageEvent, message::{MessageSender, unreturn_message::UnReturnMessage}};

use crate::message::{ManagerCommand, TaskCommand};


pub struct ManagerActor{
    config: qexed_config::app::qexed_entity::EntityConfig,
}
impl ManagerActor {
    pub fn new(config: qexed_config::app::qexed_entity::EntityConfig)->Self{
        Self { config }
    }
    
}

#[async_trait::async_trait]
impl TaskManageEvent<uuid::Uuid, UnReturnMessage<ManagerCommand>, UnReturnMessage<TaskCommand>>
    for ManagerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<ManagerCommand>>,
        task_map: &DashMap<uuid::Uuid, MessageSender<UnReturnMessage<TaskCommand>>>,
        mut data: UnReturnMessage<ManagerCommand>,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }
}