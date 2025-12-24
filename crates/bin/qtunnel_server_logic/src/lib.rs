use qexed_task::{message::return_message::ReturnMessage, task::task_manage::TaskManage};
use tokio::sync::mpsc::UnboundedSender;

use crate::{manager::ServerLogicManagerActor, message::ManagerMessage, registry::get_registry_data_packets, update_tags::get_update_tags_packet};

pub mod manager;
pub mod message;
pub mod task;
mod registry;
mod update_tags;
pub async fn run(
    config: qexed_config::app::qtunnel_server_logic::ServerLogicConfig,
    qexed_heartbeat_api:UnboundedSender<ReturnMessage<qexed_heartbeat::message::ManagerCommand>>,
    qexed_command_api:UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    qexed_player_list_api:UnboundedSender<ReturnMessage<qexed_player_list::Message>>,

) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerMessage>>> {
    let registry_data: Vec<qexed_protocol::to_client::configuration::registry_data::RegistryData> = get_registry_data_packets()?;
    let tags: qexed_protocol::to_client::configuration::tags::Tags = get_update_tags_packet()?;
    let manager_actor = ServerLogicManagerActor::new(
        config,
        registry_data,
        tags,
        qexed_heartbeat_api,
        qexed_command_api,
        qexed_player_list_api,
    );
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    manager_task.run().await?;
    log::info!("[服务] 游戏逻辑 已启用");
    Ok(manager_sender)
}
