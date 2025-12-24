use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

use crate::{manager::TcpConnectManagerActor, messages::ManagerCommand};
mod manager;
pub mod messages;
mod task;
pub async fn run(
    config: qexed_config::app::qtunnel_tcp_connect_app::TcpConnect,
    qexed_status_api: UnboundedSender<ReturnMessage<qexed_status::Message>>,
    qexed_player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    qexed_black_list_api: UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
    qexed_white_list_api: UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
    qtunnel_server_logic_api:UnboundedSender<ReturnMessage<qtunnel_server_logic::message::ManagerMessage>>,
) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerCommand>>> {
    let manager_actor = TcpConnectManagerActor::new(
        config,
        qexed_status_api,
        qexed_player_list_api,
        qexed_black_list_api,
        qexed_white_list_api,
        qtunnel_server_logic_api,
    )
    .await;
    let (manager_task, manager_sender) =
        qexed_task::task::task_manage::TaskManage::new(manager_actor);
    manager_task.run().await?;
    ReturnMessage::build(ManagerCommand::Start)
        .get(&manager_sender)
        .await?;
    log::info!("[服务] TCP连接入口 已启用");
    Ok(manager_sender)
}
