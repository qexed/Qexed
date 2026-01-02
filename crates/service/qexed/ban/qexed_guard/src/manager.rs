use qexed_config::app::qexed_guard::GuardConfig;
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{message::ManagerMessage, task::tcp_connect_start};

#[derive(Debug)]
pub struct TaskManager {
    config: GuardConfig,
    qexed_warden_api: UnboundedSender<ReturnMessage<qexed_warden::message::ManagerMessage>>,
    qexed_database_api: UnboundedSender<UnReturnMessage<qexed_database::message::ManageCommand>>,

    // 子任务表
    sub_task_tcp_connect_start:Option<MessageSender<crate::message::TcpConnectStartMessage>>,
}
impl TaskManager {
    pub fn new(
        config: GuardConfig,
        qexed_warden_api: UnboundedSender<ReturnMessage<qexed_warden::message::ManagerMessage>>,
        qexed_database_api:UnboundedSender<UnReturnMessage<qexed_database::message::ManageCommand>>
    ) -> Self {
        Self {
            config,
            qexed_warden_api,
            qexed_database_api,
            sub_task_tcp_connect_start:None,
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
            ManagerMessage::GetTcpConnectStart(_) => {

                if let Some(send) = data.get_return_send().await?{
                    let subapi = match &self.sub_task_tcp_connect_start {
                        Some(v)=>v.clone(),
                        None=>{
                            let (task, task_send) =qexed_task::task::task::Task::new(api.clone(), 
                                tcp_connect_start::TcpConnectStartSubActor::new(
                                    self.config.tcp_connect_app_start.clone(), 
                                    self.qexed_warden_api.clone(),
                                    self.qexed_database_api.clone(),
                                )
                            );
                            task.run().await?;
                            self.sub_task_tcp_connect_start = Some(task_send.clone());
                            task_send
                        
                        }
                    };
                    let _ = send.send(ManagerMessage::GetTcpConnectStart(Some(subapi)));
                }
                return Ok(false);
                
            },
            ManagerMessage::Command(ref cmd) => {
                cmd.send_chat_message("暂未完成").await?;
                return Ok(false);
            }
        }
    }
}
