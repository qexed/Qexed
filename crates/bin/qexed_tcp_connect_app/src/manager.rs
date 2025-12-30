use std::net::SocketAddr;

use dashmap::DashMap;
use qexed_config::app::qexed_tcp_connect_app::TcpConnect;
use qexed_task::{event::task_manage::TaskManageEvent, message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};
use tokio::sync::mpsc::UnboundedSender;

use crate::{listen_task::ListenTask, logic_task::LogicTask, messages::{ListenCommand, LogicCommand, ManagerCommand}};

#[derive(Debug)]
pub struct TcpConnectManagerActor {
    config: TcpConnect,
    qexed_status_api: UnboundedSender<ReturnMessage<qexed_status::Message>>,
    qexed_player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    qexed_black_list_api: UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
    qexed_white_list_api: UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
    qexed_game_logic_api: UnboundedSender<ReturnMessage<qexed_game_logic::message::ManagerMessage>>,
    listen_task:Option<UnboundedSender<UnReturnMessage<ListenCommand>>>,
}
impl TcpConnectManagerActor {
    pub async fn new(
        config: TcpConnect,
        qexed_status_api: UnboundedSender<ReturnMessage<qexed_status::Message>>,
        qexed_player_list_api: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
        qexed_black_list_api: UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
        qexed_white_list_api: UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
        qexed_game_logic_api: UnboundedSender<
            ReturnMessage<qexed_game_logic::message::ManagerMessage>,
        >,
    ) -> Self {
        Self {
            config,
            qexed_status_api,
            qexed_player_list_api,
            qexed_black_list_api,
            qexed_white_list_api,
            qexed_game_logic_api,
            listen_task:None,
        }
    }

}
#[async_trait::async_trait]
impl TaskManageEvent<SocketAddr, ReturnMessage<ManagerCommand>, UnReturnMessage<LogicCommand>> for TcpConnectManagerActor {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerCommand>>,
        task_map: &DashMap<SocketAddr, MessageSender<UnReturnMessage<LogicCommand>>>,
        mut data: ReturnMessage<ManagerCommand>,
    ) -> anyhow::Result<bool> {
        let send = match data.get_return_send().await?{
            Some(v) =>v,
            None=>{
                log::error!("非法TCP管理命令");
                return Ok(false)
            }
        };
        match data.data {
            ManagerCommand::Start => {
                let task_data = ListenTask::new(self.config.clone(), api.clone());
                let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
                task.run().await?;
                task_send.send(UnReturnMessage::build(ListenCommand::Start))?;
                self.listen_task = Some(task_send);
                let _ = send.send(data.data);
            },
            ManagerCommand::NewConnection(tcp_stream, socket_addr) => {
                log::debug!("新TCP连接:{:?},Addr:{:?}",tcp_stream,socket_addr.ip());
                // 创建逻辑任务,他将用于初始化阶段
                let actor = LogicTask::new(
                    tcp_stream,socket_addr
                );
                let (task, task_send) =
                    qexed_task::task::task::Task::new(api.clone(), actor);
                task.run().await?;
                UnReturnMessage::build(LogicCommand::Start)
                    .post(&task_send)
                    .await?;
                task_map.insert(socket_addr, task_send);
                let _ = send.send(ManagerCommand::NewConnectionFinish);
            },
            ManagerCommand::NewConnectionFinish => {
                // 此命令理论不会触发,因为这tm是给ListenTask看的
            },
        }
        Ok(false)
    }
}