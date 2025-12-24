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

#[derive(Debug)]
pub enum Message {
    PlayerJoin(uuid::Uuid, String),
    PlayerLeft(uuid::Uuid),
    CheckPlayeIsInList(uuid::Uuid, bool),
    LoadData(i32, i32),
    Command(CommandData),
    GetPlayerIsOnline{name:String,is_true:bool,player_uuid:uuid::Uuid},
}

#[derive(Debug)]
pub struct Task {
    pub player: i32,
    pub max_player: i32,
    pub player_map: DashMap<uuid::Uuid, String>,
    pub player_name_map: DashMap<String,uuid::Uuid>,
}
impl Task {
    pub fn new(config: PlayerList) -> Self {
        Self {
            player: 0,
            max_player: config.max_player,
            player_map: Default::default(),
            player_name_map: Default::default(),
        }
    }
    // 获取玩家列表分页
    fn get_players_page(&self, page: usize) -> (Vec<(Uuid, String)>, usize, usize, usize) {
        const PAGE_SIZE: usize = 20;

        // 收集所有玩家
        let mut players: Vec<(Uuid, String)> = Vec::new();
        for entry in self.player_map.iter() {
            players.push((*entry.key(), entry.value().clone()));
        }

        // 按玩家名排序
        players.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

        let total_players = players.len();
        let total_pages = if total_players > 0 {
            (total_players as f32 / PAGE_SIZE as f32).ceil() as usize
        } else {
            1
        };

        // 处理页码
        let current_page = if page == 0 { 1 } else { page };
        let current_page = current_page.min(total_pages);

        // 计算分页
        let start = (current_page - 1) * PAGE_SIZE;
        let end = (current_page * PAGE_SIZE).min(total_players);

        let page_players = if start < total_players {
            players[start..end].to_vec()
        } else {
            Vec::new()
        };

        (page_players, current_page, total_pages, total_players)
    }

    // 格式化玩家列表消息
    fn format_player_list(&self, page: usize) -> String {
        const PAGE_SIZE: usize = 20;

        let (players, current_page, total_pages, total_players) = self.get_players_page(page);

        let mut message = Vec::new();

        // 标题行
        message.push(format!(
            "§a=== 在线玩家列表 (§e{}/{}§a) ===",
            current_page, total_pages
        ));
        message.push(format!("§f在线: §a{}§f/§c{}", self.player, self.max_player));

        if players.is_empty() {
            if current_page > 1 && current_page > total_pages {
                message.push("§c页码超出范围！".to_string());
            } else {
                message.push("§7当前页面没有玩家。".to_string());
            }
        } else {
            // 玩家列表
            let start_num = (current_page - 1) * PAGE_SIZE + 1;
            for (i, (_, name)) in players.iter().enumerate() {
                let num = start_num + i;
                message.push(format!("§f{}. §a{}", num, name));
            }
        }

        // 底部提示
        if total_pages > 1 {
            if current_page < total_pages {
                message.push(format!("§7使用 §f/list {} §7查看下一页", current_page + 1));
            }
            if current_page > 1 {
                message.push(format!("§7使用 §f/list {} §7查看上一页", current_page - 1));
            }
            message.push(format!("§7使用 §f/list <1-{}> §7查看指定页", total_pages));
        }

        message.join("\n")
    }
}

#[async_trait]
impl TaskEasyEvent<ReturnMessage<Message>> for Task {
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<Message>>,
        mut data: ReturnMessage<Message>,
    ) -> anyhow::Result<bool> {
        match data.data {
            Message::PlayerJoin(ref uuid, ref name) => {
                self.player += 1;
                self.player_map.insert(*uuid, name.clone());
                self.player_name_map.insert(name.clone(), *uuid);
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            Message::PlayerLeft(uuid) => {
                self.player -= 1;
                if let Some(name) = self.player_map.remove(&uuid) {
                    self.player_name_map.remove(&name.1);
                }
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            Message::CheckPlayeIsInList(uuid, ref mut is_have) => {
                if self.player_map.contains_key(&uuid) {
                    *is_have = true;
                }
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            Message::LoadData(ref mut player, ref mut max_player) => {
                *player = self.player.clone();
                *max_player = self.max_player.clone();
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            Message::GetPlayerIsOnline{ref name,ref mut is_true,ref mut player_uuid}=>{
                if let Some(playeruuid) = self.player_name_map.get(name){
                    *is_true = true;
                    *player_uuid = playeruuid.value().clone();
                };
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            Message::Command(ref cmd) => {
                // 解析页码参数
                let args = cmd.parse_args();
                let help_args: Vec<String> = args.into_iter().skip(1).collect();
                let page = match help_args.len() {
                    0 => 1,
                    1 => {
                        // 一个参数：可能是页码或命令名
                        match help_args[0].parse::<usize>() {
                            Ok(page) => page,
                            Err(_) => 1,
                        }
                    }
                    _ => {
                        // 多个参数：错误用法
                        cmd.send_chat_message("§c用法: /list [页码]\n§7例如: /list 或 /list 1").await?;
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };

                // 生成玩家列表消息
                let message = self.format_player_list(page);

                // 发送消息
                cmd.send_chat_message(&message).await?;
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
        }
    }
}
pub async fn register_list_command(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    api2: UnboundedSender<ReturnMessage<Message>>,
) -> anyhow::Result<()> {
    // 克隆 api2 用于闭包
    let api2_for_closure = api2.clone();

    qexed_command::register::register_command(
        "list",
        "查询玩家列表",
        "qexed.list",
        vec![qexed_command::message::CommandParameter {
            name: "page".to_string(),
            description: "页码".to_string(),
            required: false,
            param_type: qexed_command::message::ParameterType::Integer {
                min: Some(0),
                max: None,
            },
            suggestions: None,
        }],
        vec!["list", "玩家列表"],
        command_api,
        move |mut cmd_rx| {
            // 使用 move 关键字
            let api2 = api2_for_closure.clone(); // 在闭包内部再克隆一次
            async move {
                // 处理命令，直到通道关闭
                while let Some(cmd) = cmd_rx.recv().await {
                    ReturnMessage::build(Message::Command(cmd))
                        .get(&api2) // 使用闭包内部的 api2
                        .await?;
                }

                // 通道关闭，正常结束
                Ok(())
            }
        },
    )
    .await
}
pub async fn run(config: PlayerList) -> anyhow::Result<UnboundedSender<ReturnMessage<Message>>> {
    let task_data = Task::new(config);
    let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
    task.run().await?;
    log::info!("[服务] 玩家列表 已启用");
    Ok(task_send)
}
