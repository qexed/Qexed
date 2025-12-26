use qexed_config::app::qexed_one::One;
use qexed_task::message::{return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

pub struct Api {
    /// 玩家数统计服务
    pub player_list: UnboundedSender<ReturnMessage<qexed_player_list::Message>>,
    /// 玩家黑名单
    pub black_list: UnboundedSender<ReturnMessage<qexed_blacklist::Message>>,
    /// 玩家白名单
    pub white_list: UnboundedSender<ReturnMessage<qexed_whitelist::Message>>,
    /// 玩家ping信息构建服务
    pub server_status: UnboundedSender<ReturnMessage<qexed_status::Message>>,
    /// 玩家连接维护服务
    pub tcp_connect:
        UnboundedSender<ReturnMessage<qexed_tcp_connect_app::messages::ManagerCommand>>,
    /// 玩家Ping维持服务
    pub ping: UnboundedSender<ReturnMessage<qexed_ping::message::ManagerCommand>>,
    /// 玩家核心逻辑服务
    pub game_logic: UnboundedSender<ReturnMessage<qexed_game_logic::message::ManagerMessage>>,
    /// 实体id分配器服务
    pub entity_id_allocator: UnboundedSender<ReturnMessage<qexed_entity_id_allocator::Message>>,
    /// 数据包分流服务
    pub packet_split: UnboundedSender<ReturnMessage<qexed_packet_split::message::ManagerMessage>>,
    /// 心跳服务
    pub heartbeat: UnboundedSender<ReturnMessage<qexed_heartbeat::message::ManagerCommand>>,
    /// 聊天服务
    pub chat: UnboundedSender<ReturnMessage<qexed_chat::message::ManagerMessage>>,
    /// 指令服务
    pub command: UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    /// 规则服务
    pub rule: qexed_shared::Shared<qexed_config::app::qexed_rule::RuleConfig>,
    /// 区块服务
    pub chunk: UnboundedSender<UnReturnMessage<qexed_chunk::message::world::WorldCommand>>,
    /// Title指令服务
    pub title:UnboundedSender<ReturnMessage<qexed_title::message::ManagerMessage>>,
}
impl Api {
    pub async fn init(config: One) -> anyhow::Result<Self> {
        let command = qexed_command::run(config.command).await?;
        let player_list = qexed_player_list::run(config.player_list).await?;
        let black_list = qexed_blacklist::run(config.black_list).await?;
        let white_list = qexed_whitelist::run(config.white_list).await?;
        let server_status = qexed_status::run(config.server_status, player_list.clone()).await?;
        let ping = qexed_ping::run(config.ping).await?;
        let heartbeat = qexed_heartbeat::run(config.heartbeat).await?;
        let chat = qexed_chat::run(config.chat,player_list.clone()).await?;
        let title = qexed_title::run(config.title, player_list.clone()).await?;
        let packet_split = qexed_packet_split::run(config.packet_split).await?;
        let chunk = qexed_chunk::run(config.chunk).await?;
        let game_logic = qexed_game_logic::run(
            config.game_logic,
            ping.clone(),
            heartbeat.clone(),
            packet_split.clone(),
            chat.clone(),
            command.clone(),
            player_list.clone(),
            chunk.clone(),
            title.clone(),
        )
        .await?;
        let tcp_connect = qexed_tcp_connect_app::run(
            config.tcp_connect_app,
            server_status.clone(),
            player_list.clone(),
            black_list.clone(),
            white_list.clone(),
            game_logic.clone(),
        )
        .await?;
        let entity_id_allocator =
            qexed_entity_id_allocator::run(config.entity_id_allocator).await?;
        let rule = qexed_rule::run(config.rule).await?;
        
        Ok(Self {
            player_list,
            black_list: black_list,
            white_list: white_list,
            server_status: server_status,
            tcp_connect: tcp_connect,
            game_logic: game_logic,
            ping: ping,
            entity_id_allocator: entity_id_allocator,
            packet_split: packet_split,
            heartbeat: heartbeat,
            chat: chat,
            command: command,
            rule: rule,
            chunk: chunk,
            title:title,
        })
    }
    pub async fn _listen() -> anyhow::Result<()> {
        Ok(())
    }
    pub async fn register(&self) -> anyhow::Result<()> {
        qexed_player_list::register_list_command(&self.command, self.player_list.clone()).await?;
        qexed_chat::command::register_tell_command(&self.command, self.chat.clone()).await?;
        qexed_chat::command::register_me_command(&self.command, self.chat.clone()).await?;
        qexed_chat::command::register_say_command(&self.command, self.chat.clone()).await?;
        qexed_chunk::command::seed::register_seed_command(&self.command, self.chunk.clone()).await?;
        qexed_title::command::register_title_command_full(&self.command, self.title.clone()).await?;
        Ok(())
    }
}
