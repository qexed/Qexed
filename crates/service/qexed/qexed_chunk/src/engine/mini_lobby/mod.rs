use qexed_task::message::{MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

use crate::{engine::mini_lobby::event::world::WorldManage, message::world::WorldCommand};

pub mod event;
pub mod task;

pub async fn run(
    config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
    qexed_entity_api:UnboundedSender<ReturnMessage<qexed_entity::message::ManagerCommand>>,
) -> anyhow::Result<UnboundedSender<UnReturnMessage<WorldCommand>>> {
    let manager_actor =WorldManage::new(config,qexed_entity_api);
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);

    manager_task.run().await?;
    manager_sender.send(UnReturnMessage::build(WorldCommand::Init))?;
    log::info!("[服务:区块] 引擎:迷你大厅");
    Ok(manager_sender)
}