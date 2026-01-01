use async_trait::async_trait;
use bytes::Bytes;
use qexed_task::message::unreturn_message::UnReturnMessage;
use qexed_tcp_connect::PacketSend;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum ManagerCommand {
    NewPlayerConnect(
        uuid::Uuid,
        String,
        bool,
        Option<NewPlayerConnectError>,
        Option<UnboundedSender<UnReturnMessage<TaskCommand>>>,
        Option<UnboundedSender<Bytes>>,
    ),
    Command(String),
    RegisterCommand {
        name: String,
        doc: String,
        permission: String,
        parameters: Vec<CommandParameter>, // 新增参数定义
        aliases: Vec<String>,              // 新增别名
        api: Option<UnboundedSender<CommandData>>,
        success: bool,
    },
    GetCommandPacket(uuid::Uuid, Option<Bytes>),
    GetCommand(String, Option<UnboundedSender<CommandData>>),
    PlayerClose(uuid::Uuid),
    // 新增：Tab补全请求
    TabComplete {
        player_uuid: uuid::Uuid,
        command_line: String,
        cursor: usize,
        suggestions: Vec<String>,
    },
    CommandHelp(CommandData),
}
#[derive(Debug)]
pub enum TaskCommand {
    Command(String),
    InitCommandPacket,
    Close,
}
#[derive(Debug)]
pub struct CommandData {
    pub player_uuid: Option<uuid::Uuid>,
    pub player_name: Option<String>,
    pub command_line: String,
    pub is_cmd: bool,
    pub packet_sender: Option<UnboundedSender<Bytes>>,
}
impl CommandData {
    pub fn new(
        player_uuid: Option<uuid::Uuid>,
        player_name: Option<String>,
        command_line: String,
        is_cmd: bool,
        packet_sender: Option<UnboundedSender<Bytes>>,
    ) -> Self {
        Self {
            player_uuid,
            player_name,
            command_line,
            packet_sender,
            is_cmd,
        }
    }

    /// 发送聊天消息给玩家
    pub async fn send_chat_message(&self, message: &str) -> anyhow::Result<()> {
        // 根据来源选择不同的输出方式
        if self.is_cmd {
            // 控制台：使用日志输出
            let lines = self.strip_format_codes(message);
            for line in lines.split('\n') {
                if !line.trim().is_empty() {
                    log::info!("{}", line);
                }
            }
        } else {
            // 玩家：发送聊天消息
            let mut chat_component = std::collections::HashMap::new();
            chat_component.insert(
                "text".to_string(),
                qexed_nbt::Tag::String(message.into()), // 使用 `into()` 转为 Arc<str>
            );
            let content_nbt = qexed_nbt::Tag::Compound(std::sync::Arc::new(chat_component));

            if let Some(packet_sender) = &self.packet_sender {
                // 发送数据包
                packet_sender
                    .send(
                        PacketSend::build_send_packet(
                            qexed_protocol::to_client::play::system_chat::SystemChat {
                                content: content_nbt,
                                overlay: false,
                            },
                        )
                        .await?,
                    )
                    .map_err(|e| anyhow::anyhow!("发送聊天消息失败: {}", e))?;
            }
        }

        Ok(())
    }

    /// 解析命令参数
    pub fn parse_args(&self) -> Vec<String> {
        shlex::split(&self.command_line).unwrap_or_else(|| {
            // 如果解析失败，使用简单空格分割
            self.command_line
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        })
    }
    /// 去除Minecraft格式代码（§符号及其后的字符）
    fn strip_format_codes(&self, text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '§' {
                // 跳过§及其后的一个字符（格式代码）
                chars.next(); // 跳过格式代码
            } else {
                result.push(c);
            }
        }

        result
    }
}
#[derive(Debug)]
pub struct RegisterCommand {
    pub api: tokio::sync::mpsc::UnboundedSender<crate::message::CommandData>,
    pub name: String,
    pub doc: String,
    pub permission: String,
    pub parameters: Vec<CommandParameter>, // 新增：参数定义
    pub aliases: Vec<String>,              // 新增：命令别名
}
// 新增参数类型定义
#[derive(Debug, Clone)]
pub enum ParameterType {
    Literal(&'static str), // 字面量，如"about", "version"
    String { behavior: StringBehavior },
    Integer { min: Option<i32>, max: Option<i32> },
    Boolean,
    Player,
    // 可以添加更多类型
}

#[derive(Debug, Clone)]
pub enum StringBehavior {
    SingleWord,
    Quotable,
    Greedy,
}

// 参数定义
#[derive(Debug, Clone)]
pub struct CommandParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub param_type: ParameterType,
    pub suggestions: Option<Vec<String>>,
}
#[derive(Error, Debug, Clone)]
pub enum NewPlayerConnectError {
    #[error("玩家已存在")]
    PlayerAlreadyExists,
    #[error("无效的连接通道")]
    InvalidConnection,
}
