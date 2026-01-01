use qexed_config::app::qexed_guard::GuardConfig;
use qexed_task::{event::task::TaskEasyEvent, message::{MessageSender, return_message::ReturnMessage}};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::ManagerMessage;

#[derive(Debug)]
pub struct TaskManager {
    config: GuardConfig,
    qexed_warden_api:UnboundedSender<ReturnMessage<qexed_warden::message::ManagerMessage>>,

}
impl TaskManager {
    pub fn new(config: GuardConfig,qexed_warden_api:UnboundedSender<ReturnMessage<qexed_warden::message::ManagerMessage>>) -> Self {
        Self {
            config,qexed_warden_api
        }
    }
}

#[async_trait::async_trait]
impl TaskEasyEvent<ReturnMessage<ManagerMessage>> for TaskManager {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerMessage>>,
        mut data: ReturnMessage<ManagerMessage>,
    ) -> anyhow::Result<bool> {
        match data.data {

            ManagerMessage::Command(ref cmd) => {
                cmd.send_chat_message("暂未完成").await?;
                return Ok(false);
            }
        }
    }
}
