use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;

use crate::{manager::PacketSplitManagerActor, message::ManagerMessage};
pub mod manager;
pub mod message;
pub mod task;
pub async fn run(
    config: qexed_config::app::qexed_packet_split::PacketSplitConfig,
) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerMessage>>> {
    let manager_actor = PacketSplitManagerActor::new( 
        config,
    );
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    manager_task.run().await?;
    log::info!("[服务] 数据包分流 已启用");
    Ok(manager_sender)
}
