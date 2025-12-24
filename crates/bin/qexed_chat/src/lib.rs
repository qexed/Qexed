use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;

use crate::{manager::ChatManagerActor, message::ManagerMessage};
pub mod command;
pub mod manager;
pub mod message;
pub mod task;
pub async fn run(
    config: qexed_config::app::qexed_chat::ChatConfig,
    player_list_api:UnboundedSender<ReturnMessage<qexed_player_list::Message>>

) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerMessage>>> {
    let manager_actor = ChatManagerActor::new(
        config,
        player_list_api
    );
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    manager_task.run().await?;
    log::info!("[服务] 聊天 已启用");
    Ok(manager_sender)
}
