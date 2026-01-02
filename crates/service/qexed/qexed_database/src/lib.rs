use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;

pub mod manage;
pub mod message;
pub mod task;
pub async fn run(

) -> anyhow::Result<UnboundedSender<ReturnMessage<message::ManageCommand>>> {
    let manager_actor = manage::ManageActor::new();
    let (manager_task, manager_sender) =
        qexed_task::task::task::TaskEasy::new(manager_actor);
    
    manager_task.run().await?;
    log::info!("[服务] 数据库 已启用");
    Ok(manager_sender)
}