use qexed_config::app::{qtunnel_one::One};
use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;

pub struct Api {
    /// 玩家数统计服务
    pub player_list: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    /// 玩家黑名单
    pub black_list:UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
    /// 玩家白名单
    pub white_list:UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
    /// 玩家ping信息构建服务
    pub server_status:UnboundedSender<ReturnMessage<qexed_status::Message>>,
    /// 心跳服务 
    pub heartbeat:UnboundedSender<ReturnMessage<qexed_heartbeat::message::ManagerCommand>>,
    /// 指令服务
    pub command:UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    /// tcp连接服务
    pub tcp_connect_app:UnboundedSender<ReturnMessage<qtunnel_tcp_connect_app::messages::ManagerCommand>>,
}
impl Api {
    pub async fn init(config: One) -> anyhow::Result<Self> {
        let command = qexed_command::run(config.command).await?;
        let player_list= qexed_player_list::run(config.player_list).await?;
        let black_list= qexed_blacklist::run(config.black_list).await?;
        let white_list= qexed_whitelist::run(config.white_list).await?;
        let server_status = qexed_status::run(config.server_status, player_list.clone()).await?;
        let heartbeat = qexed_heartbeat::run(config.heartbeat).await?;
        let server_logic = qtunnel_server_logic::run(config.server_logic,heartbeat.clone(),command.clone(),player_list.clone()).await?;
        let tcp_connect_app = qtunnel_tcp_connect_app::run(config.tcp_connect_app,server_status.clone(),player_list.clone(),black_list.clone(),white_list.clone(),server_logic.clone()).await?;
        Ok(Self {
            player_list,
            black_list: black_list,
            white_list:white_list,
            server_status:server_status,
            command:command,
            tcp_connect_app:tcp_connect_app,
            heartbeat:heartbeat,
        })
    }
    pub async fn _listen()->anyhow::Result<()>{
        Ok(())
    }
    pub async fn register(&self)->anyhow::Result<()>{
        qexed_player_list::register_list_command(&self.command, self.player_list.clone()).await?;
        Ok(())
    }
}
