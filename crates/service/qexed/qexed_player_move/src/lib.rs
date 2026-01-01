// use qexed_task::message::return_message::ReturnMessage;
// use tokio::sync::mpsc::UnboundedSender;

// use crate::{manage::TitleManagerActor, message::ManagerMessage};

// pub mod task;
// pub mod manage;
// pub mod message;
// pub mod command;


// pub async fn run(
//     config: qexed_config::app::qexed_title::TitleConfig,

// ) -> anyhow::Result<UnboundedSender<ReturnMessage<ManagerMessage>>> {
//     let manager_actor = TitleManagerActor::new(
//         config,
//         player_list_api
//     );
//     let (manager_task, manager_sender) =
//         qexed_task::task::task_manage::TaskManage::new(manager_actor);
//     manager_task.run().await?;
//     log::info!("[服务] Title 已启用");
//     Ok(manager_sender)
// }
