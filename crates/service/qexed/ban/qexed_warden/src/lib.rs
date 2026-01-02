use async_trait::async_trait;
use dashmap::DashMap;
use qexed_command::message::CommandData;
use qexed_config::app::qexed_player_list::PlayerList;
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage},
};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::message::ManagerMessage;
pub mod manager;
pub mod command;
pub mod message;
pub async fn run(config: qexed_config::app::qexed_wardon::WardonConfig,
    qexed_database_api:UnboundedSender<ReturnMessage<qexed_database::message::ManageCommand>>
) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerMessage>>> {
    let task_data = manager::TaskManager::new(config);
    let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
    task.run().await?;
    log::info!("[服务] 典狱长 已启用");
    Ok(task_send)
}
