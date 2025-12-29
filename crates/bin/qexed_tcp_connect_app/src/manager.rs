use std::net::SocketAddr;

use dashmap::DashMap;
use qexed_config::app::qexed_tcp_connect_app::TcpConnect;
use qexed_task::{event::task_manage::TaskManageEvent, message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};
use tokio::sync::mpsc::UnboundedSender;

use crate::{listen_task::ListenTask, messages::{ListenCommand, LogicCommand, ManagerCommand}};

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
impl TaskManageEvent<SocketAddr, ReturnMessage<ManagerCommand>, ReturnMessage<LogicCommand>> for TcpConnectManagerActor {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerCommand>>,
        task_map: &DashMap<SocketAddr, MessageSender<ReturnMessage<LogicCommand>>>,
        mut data: ReturnMessage<ManagerCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            ManagerCommand::Start => {
                let task_data = ListenTask::new(self.config.clone(), api.clone());
                let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
                task.run().await?;
                task_send.send(UnReturnMessage::build(ListenCommand::Start))?;
                self.listen_task = Some(task_send);
                if let Some(send)=data.get_return_send().await?{
                    let _ = send.send(data.data);
                }
            },
            ManagerCommand::NewConnection(ref tcp_stream, ref socket_addr) => {
                log::debug!("新TCP连接:{:?},Addr:{:?}",tcp_stream,socket_addr.ip());
                if let Some(send)=data.get_return_send().await?{
                    let _ = send.send(data.data);
                }
            },
        }
        Ok(false)
    }
}