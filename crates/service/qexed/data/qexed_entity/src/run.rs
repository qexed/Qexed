use qexed_task::message::{MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::{manage, message::ManagerCommand};

pub async fn run(
    config: qexed_config::app::qexed_entity::EntityConfig,
) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerCommand>>> {
    let manager_actor = manage::ManagerActor::new(config);
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    
    manager_task.run().await?;
    ReturnMessage::build(ManagerCommand::Start).get(&manager_sender).await?;
    log::info!("[服务] 实体 已启用");
    // Err(anyhow::anyhow!("实体模块开发中暂时不可用"))
    Ok(manager_sender)
}