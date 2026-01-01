use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;

pub mod manager;
pub mod message;
pub mod task;

pub async fn run(
    config: qexed_config::app::qexed_heartbeat::HeartbeatConfig,
) -> anyhow::Result<UnboundedSender<ReturnMessage<message::ManagerCommand>>> {
    let manager_actor = manager::HeartbeatManagerActor::new(config);
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    
    manager_task.run().await?;
    log::info!("[服务] 心跳 已启用");
    
    Ok(manager_sender)
}