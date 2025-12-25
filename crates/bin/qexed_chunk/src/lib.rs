use qexed_task::message::{MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

pub mod event;
pub mod message;
pub mod engine;
pub mod data_type;
pub mod command;


pub async fn run(
    config: qexed_config::app::qexed_chunk::ChunkConfig,
) -> anyhow::Result<UnboundedSender<UnReturnMessage<message::global::GlobalCommand>>> {
    let manager_actor =event::global::GlobalManage::new(config);
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    
    manager_task.run().await?;
    manager_sender.send(UnReturnMessage::build(message::global::GlobalCommand::Init))?;
    log::info!("[服务] 区块 已启用");
    Ok(manager_sender)
}