use std::cell::RefCell;

use bytes::Bytes;
use dashmap::DashMap;
use qexed_protocol::to_client::play::commands::{Commands, Node};
use qexed_task::event::task::TaskEvent;
use qexed_task::message::MessageType;
use qexed_task::message::return_message::ReturnMessage;
use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use qexed_tcp_connect::PacketSend;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::message::{CommandData, ManagerCommand, TaskCommand};

#[derive(Debug)]
pub struct CommandTask {
    config: qexed_config::app::qexed_command::CommandConfig,
    player_uuid: Uuid,
    player_name: String,
    packet_send: UnboundedSender<Bytes>,
    // 命令发送器缓存：命令名 -> 发送器 (使用 RefCell 实现内部可变性)
    cmd_cache: DashMap<String, UnboundedSender<CommandData>>,
}

impl CommandTask {
    pub fn new(
        config: qexed_config::app::qexed_command::CommandConfig,
        player_uuid: Uuid,
        player_name: String,
        packet_send: UnboundedSender<Bytes>,
    ) -> Self {
        Self {
            player_uuid,
            packet_send,
            player_name,
            config,
            cmd_cache: DashMap::new(),
        }
    }

    /// 从缓存获取或向管理器请求命令发送器
    async fn get_cmd_sender(
        &self,
        cmd_name: &str,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
    ) -> anyhow::Result<Option<UnboundedSender<CommandData>>> {
        // 1. 检查缓存
        {
            let cache = &self.cmd_cache;
            if let Some(sender) = cache.get(cmd_name) {
                return Ok(Some(sender.clone()));
            }
        }
        let result = ReturnMessage::build(ManagerCommand::GetCommand(cmd_name.to_string(), None))
            .get(manage_api)
            .await?;

        // 3. 处理管理器响应
        match result {
            ManagerCommand::GetCommand(_, cmd_sender) => {
                // 管理器确认有该命令，等待发送器
                match cmd_sender {
                    Some(cmd_sender) => {
                        // 存入缓存
                        self.cmd_cache
                            .insert(cmd_name.to_string(), cmd_sender.clone());
                        Ok(Some(cmd_sender))
                    }
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// 清理缓存中的特定命令发送器
    fn invalidate_cache(&self, cmd_name: &str) {
        self.cmd_cache.remove(cmd_name);
        log::debug!("已清理命令 {} 的缓存", cmd_name);
    }
}

#[async_trait::async_trait]
impl TaskEvent<UnReturnMessage<TaskCommand>, ReturnMessage<ManagerCommand>> for CommandTask {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<TaskCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: UnReturnMessage<TaskCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskCommand::InitCommandPacket => {
                if let Ok(ManagerCommand::GetCommandPacket(_,packet)) = ReturnMessage::build(ManagerCommand::GetCommandPacket(self.player_uuid,None))
                    .get(manage_api)
                    .await{
                    if let Some(packet) = packet{
                        self.packet_send.send(packet)?;
                    }
                };
            }
            TaskCommand::Command(full_cmd) => {
                log::info!(
                    "玩家 {}[{}] 执行指令: {}",
                    self.player_name,
                    self.player_uuid,
                    full_cmd
                );

                // 提取基础命令
                let base_cmd = full_cmd
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                if base_cmd.is_empty() {
                    self.send_chat_message("§c错误：空命令").await?;
                    return Ok(false);
                }

                // 处理本地命令
                if self.handle_local_command(&base_cmd, &full_cmd).await? {
                    return Ok(false);
                }

                // 获取或请求命令发送器
                match self.get_cmd_sender(&base_cmd, manage_api).await {
                    Ok(Some(cmd_sender)) => {
                        // 发送命令数据到处理器
                        let cmd_data = CommandData::new(
                            Some(self.player_uuid),
                            Some(self.player_name.clone()),
                            full_cmd.clone(),
                            false, // 假设有权限
                            Some(self.packet_send.clone()),
                        );

                        if let Err(e) = cmd_sender.send(cmd_data) {
                            log::error!("发送命令数据失败: {}，清理缓存", e);
                            self.invalidate_cache(&base_cmd);
                        }
                    }
                    Ok(None) => {
                        // 命令不存在
                        self.send_chat_message(&format!("§c未知命令: /{}", base_cmd))
                            .await?;
                        self.send_chat_message("§7输入 /help 查看可用命令").await?;
                    }
                    Err(e) => {
                        log::error!("获取命令发送器失败: {}", e);
                        self.send_chat_message("§c命令系统错误，请稍后重试").await?;
                    }
                }
            }

            TaskCommand::Close => {
                // 通知管理器玩家连接关闭
                let _ = ReturnMessage::build(ManagerCommand::PlayerClose(self.player_uuid))
                    .get(manage_api)
                    .await;

                // 清理所有缓存
                self.cmd_cache.clear();
                log::info!(
                    "关闭命令任务，玩家: {}[{}]",
                    self.player_name,
                    self.player_uuid
                );
                return Ok(true);
            }
        }

        Ok(false)
    }
}

// 扩展方法：发送聊天消息
impl CommandTask {
    async fn send_chat_message(&self, message: &str) -> anyhow::Result<()> {
        let mut chat_component = std::collections::HashMap::new();
        chat_component.insert(
            "text".to_string(),
            qexed_nbt::Tag::String(message.into()), // 使用 `into()` 转为 Arc<str>
        );
        let content_nbt = qexed_nbt::Tag::Compound(std::sync::Arc::new(chat_component));
        self.packet_send.send(
            PacketSend::build_send_packet(
                qexed_protocol::to_client::play::system_chat::SystemChat {
                    content: content_nbt,
                    overlay: false,
                },
            )
            .await?,
        )?;
        Ok(())
    }

    async fn handle_local_command(&self, base_cmd: &str, full_cmd: &str) -> anyhow::Result<bool> {
        // 实现本地命令处理逻辑
        Ok(false)
    }
}
