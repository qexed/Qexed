use std::{collections::HashMap, fs::Permissions};

use async_trait::async_trait;
use dashmap::DashMap;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::commands::{Commands, Node, Varies};
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
    task::task::Task,
};
use qexed_tcp_connect::PacketSend;
use uuid::Uuid;

use crate::{
    message::{
        CommandData, ManagerCommand, NewPlayerConnectError, ParameterType, RegisterCommand,
        StringBehavior, TaskCommand,
    },
    task::CommandTask,
};

#[derive(Debug)]
pub struct CommandManagerActor {
    config: qexed_config::app::qexed_command::CommandConfig,
    cmd: HashMap<String, crate::message::RegisterCommand>,
    tab_completions: HashMap<String, Vec<String>>,
}

impl CommandManagerActor {
    pub fn new(config: qexed_config::app::qexed_command::CommandConfig) -> Self {
        Self {
            config,
            cmd: Default::default(),
            tab_completions: Default::default(),
        }
    }
    pub fn build_commands_from_list(&self, player_uuid: Uuid) -> anyhow::Result<Commands> {
        let mut nodes = Vec::new();
        let mut root_children = Vec::new();

        // 为每个命令创建节点
        for (i, (cmd_name, cmd_info)) in self.cmd.iter().enumerate() {
            // 命令节点索引
            let cmd_node_index = 1 + nodes.len() as i32;

            // 1. 创建命令字面量节点
            let mut cmd_flags = 0x01; // 字面量

            // 如果没有参数，命令本身可执行
            if cmd_info.parameters.is_empty() {
                cmd_flags |= 0x04; // 可执行
            }

            let cmd_node = Node {
                flags: cmd_flags,
                children: if cmd_info.parameters.is_empty() {
                    vec![]
                } else {
                    vec![VarInt(cmd_node_index + 1)]
                },
                redirect_node: None,
                name: Some(cmd_name.clone()),
                parser_id: None,
                properties: None,
                suggestions_type: None,
            };

            nodes.push(cmd_node);
            root_children.push(VarInt(cmd_node_index));

            // 2. 为每个参数创建节点
            let mut current_parent = cmd_node_index;
            for (param_idx, param) in cmd_info.parameters.iter().enumerate() {
                let is_last = param_idx == cmd_info.parameters.len() - 1;
                let param_node_index = cmd_node_index + 1 + param_idx as i32;

                // 创建参数节点
                let mut param_flags = 0x02; // 参数节点

                if !param.required {
                    param_flags |= 0x10; // 可选
                }

                if is_last {
                    param_flags |= 0x04; // 可执行
                }

                // 将参数类型转换为网络格式
                let (parser_id, properties) = self.parameter_to_network(&param.param_type);

                let param_node = Node {
                    flags: param_flags,
                    children: if is_last {
                        vec![]
                    } else {
                        vec![VarInt(param_node_index + 1)]
                    },
                    redirect_node: None,
                    name: Some(param.name.clone()),
                    parser_id: Some(VarInt(parser_id)),
                    properties,
                    suggestions_type: if param.suggestions.is_some() {
                        Some("minecraft:ask_server".to_string())
                    } else {
                        None
                    },
                };

                nodes.push(param_node);
                current_parent = param_node_index;
            }

            // 3. 为命令别名创建节点
            for alias in &cmd_info.aliases {
                let alias_node_index = 1 + nodes.len() as i32;

                let alias_node = Node {
                    flags: 0x09, // 字面量(0x01) + 可执行(0x04) + 重定向(0x08)
                    children: vec![],
                    redirect_node: Some(VarInt(cmd_node_index)),
                    name: Some(alias.clone()),
                    parser_id: None,
                    properties: None,
                    suggestions_type: None,
                };

                nodes.push(alias_node);
                root_children.push(VarInt(alias_node_index));
            }
        }

        // 创建根节点
        let root_node = Node {
            flags: 0x00,
            children: root_children,
            redirect_node: None,
            name: None,
            parser_id: None,
            properties: None,
            suggestions_type: None,
        };

        // 将根节点插入到开头
        nodes.insert(0, root_node);

        Ok(Commands {
            nodes,
            root_index: VarInt(0),
        })
    }

    /// 将参数类型转换为网络格式
    fn parameter_to_network(&self, param_type: &ParameterType) -> (i32, Option<Varies>) {
        use qexed_protocol::to_client::play::commands::{Brigadier, BrigadierString};

        match param_type {
            ParameterType::Literal(literal) => {
                // 字面量参数实际上是字符串的一种
                (
                    5,
                    Some(Varies::BrigadierString(BrigadierString {
                        behavior: VarInt(0), // 单个单词
                    })),
                )
            }

            ParameterType::String { behavior } => {
                let behavior_id = match behavior {
                    StringBehavior::SingleWord => 0,
                    StringBehavior::Quotable => 1,
                    StringBehavior::Greedy => 2,
                };

                (
                    5,
                    Some(Varies::BrigadierString(BrigadierString {
                        behavior: VarInt(behavior_id),
                    })),
                )
            }

            ParameterType::Integer { min, max } => {
                let mut flags = 0;
                let mut brigadier = Brigadier {
                    flags: 0,
                    min: None,
                    max: None,
                };

                if min.is_some() {
                    flags |= 0x01;
                    brigadier.min = *min;
                }

                if max.is_some() {
                    flags |= 0x02;
                    brigadier.max = *max;
                }

                brigadier.flags = flags;

                (3, Some(Varies::BrigadierInteger(brigadier)))
            }

            ParameterType::Boolean => {
                (0, None) // brigadier:bool
            }

            ParameterType::Player => {
                (6, None) // minecraft:entity
            }

            _ => {
                // 默认使用字符串类型
                (
                    5,
                    Some(Varies::BrigadierString(BrigadierString {
                        behavior: VarInt(0),
                    })),
                )
            }
        }
    }
}
#[async_trait]
impl TaskManageEvent<uuid::Uuid, ReturnMessage<ManagerCommand>, UnReturnMessage<TaskCommand>>
    for CommandManagerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManagerCommand>>,
        task_map: &DashMap<uuid::Uuid, MessageSender<UnReturnMessage<TaskCommand>>>,
        mut data: ReturnMessage<ManagerCommand>,
    ) -> anyhow::Result<bool> {
        let send = match data.get_return_send().await? {
            Some(send) => send,
            None => return Ok(false),
        };

        match data.data {
            ManagerCommand::Command(ref full_cmd) => {
                let base_cmd = full_cmd.split_whitespace().next().unwrap_or("").to_string();

                if base_cmd.is_empty() {
                    log::warn!("收到空命令或无效命令: '{}'", full_cmd);
                    // 可以返回错误响应，这里简单返回false表示不继续处理
                    let _ = send.send(data.data);
                    return Ok(false);
                }

                // 查找基础命令对应的处理器
                if self.cmd.contains_key(&base_cmd) {
                    if let Some(cmd_api) = self.cmd.get(&base_cmd) {
                        cmd_api.api.send(CommandData::new(
                            None,
                            None,
                            full_cmd.to_string(),
                            true,
                            None,
                        ))?;
                    } else {
                        log::warn!("指令:{} 执行体丢失，执行失败", base_cmd);
                    }
                } else {
                    log::warn!("指令:{} 不存在,您可以输入 help 查询可用指令", base_cmd);
                }
                let _ = send.send(data.data);
                Ok(false)
            }
            ManagerCommand::RegisterCommand {
                name,
                doc,
                permission,
                parameters,
                aliases,
                api,
                success,
            } => {
                if self.cmd.contains_key(&name) {
                    if let Some(api) = api {
                        // 关闭旧的API通道
                        let _ = api.send(CommandData::new(None, None, "".to_string(), true, None));
                    }
                    let _ = send.send(ManagerCommand::RegisterCommand {
                        name:name.clone(),
                        doc: "".to_string(),
                        permission: "".to_string(),
                        parameters: vec![],
                        aliases: vec![],
                        api: None,
                        success: false,
                    });
                    return Ok(false);
                }

                if let Some(api) = api {
                    self.cmd.insert(
                        name.clone(),
                        RegisterCommand {
                            api:api.clone(),
                            name:name.clone(),
                            doc: doc.clone(),
                            permission: permission.clone(),
                            parameters: parameters.clone(),
                            aliases: aliases.clone(),
                        },
                    );
                    for i in aliases.clone(){
                        self.cmd.insert(
                            i.clone(),
                            RegisterCommand {
                                api:api.clone(),
                                name:name.clone(),
                                doc: doc.clone(),
                                permission: permission.clone(),
                                parameters: parameters.clone(),
                                aliases: aliases.clone(),
                            },
                        ); 
                    }

                    let _ = send.send(ManagerCommand::RegisterCommand {
                        name:name.clone(),
                        doc: "".to_string(),
                        permission: "".to_string(),
                        parameters: vec![],
                        aliases: vec![],
                        api: None,
                        success: true,
                    });

                    log::info!("注册命令: {}", name);
                } else {
                    let _ = send.send(ManagerCommand::RegisterCommand {
                        name:name.clone(),
                        doc: "".to_string(),
                        permission: "".to_string(),
                        parameters: vec![],
                        aliases: vec![],
                        api: None,
                        success: false,
                    });
                }

                Ok(false)
            }

            ManagerCommand::GetCommand(ref name, ref mut cmd_api) => {
                if self.cmd.contains_key(name) {
                    if let Some(register_cmd_api) = self.cmd.get(name) {
                        *cmd_api = Some(register_cmd_api.api.clone())
                    }
                }
                let _ = send.send(data.data);
                return Ok(false);
            }
            ManagerCommand::GetCommandPacket(player_uuid, _) => {
                let packet = self.build_commands_from_list(player_uuid.clone())?;
                let _ = send.send(ManagerCommand::GetCommandPacket(
                    player_uuid,
                    Some(PacketSend::build_send_packet(packet).await?),
                ));
                return Ok(false);
            }
            ManagerCommand::NewPlayerConnect(
                ref mut uuid,
                ref mut username,
                _is_true,
                _err,
                _task_api,
                mut packet_send,
            ) => {
                // 检查玩家是否已存在
                if task_map.contains_key(&uuid) {
                    let _ = send.send(ManagerCommand::NewPlayerConnect(
                        uuid.clone(),
                        "".to_string(),
                        false,
                        Some(NewPlayerConnectError::PlayerAlreadyExists.into()),
                        None,
                        None,
                    ));
                    return Ok(false);
                }

                // 获取数据包发送通道
                let packet_send: tokio::sync::mpsc::UnboundedSender<bytes::Bytes> =
                    match packet_send.take() {
                        Some(pk) => pk,
                        None => {
                            let _ = send.send(ManagerCommand::NewPlayerConnect(
                                *uuid,
                                "".to_string(),
                                false,
                                None,
                                None,
                                None,
                            ));
                            return Ok(false);
                        }
                    };

                // 创建心跳任务
                let t = CommandTask::new(
                    self.config.clone(),
                    uuid.clone(),
                    username.clone(),
                    packet_send,
                );
                let (task, task_sand) = Task::new(api.clone(), t);
                task.run().await?;

                // 保存任务通道
                task_map.insert(*uuid, task_sand.clone());

                // 返回成功
                let _ = send.send(ManagerCommand::NewPlayerConnect(
                    *uuid,
                    "".to_string(),
                    true,
                    None,
                    Some(task_sand),
                    None,
                ));

                Ok(false)
            }
            ManagerCommand::CommandHelp(ref command_data) => {

                // 解析帮助命令参数
                let args = command_data.parse_args();

                // 移除命令名本身（"help"），获取真正的参数
                let help_args: Vec<String> = args.into_iter().skip(1).collect();

                // 检查是否来自控制台（通过player_uuid或player_name判断）
                let is_console = command_data.is_cmd;

                let response = match help_args.len() {
                    0 => {
                        // 没有参数：显示第一页
                        generate_paginated_help(
                            1,
                            is_console,
                            &self.cmd,
                        )
                    }
                    1 => {
                        // 一个参数：可能是页码或命令名
                        match help_args[0].parse::<usize>() {
                            Ok(page) => {
                                // 参数是数字：显示指定页码
                                generate_paginated_help(
                                    page,
                                    is_console,
                                    &self.cmd,
                                )
                            }
                            Err(_) => {
                                // 参数不是数字：当作命令名处理
                                generate_command_help(
                                    &help_args[0],
                                    &self.cmd,
                                )
                            }
                        }
                    }
                    _ => {
                        // 多个参数：错误用法
                        "§c用法: /help [页码|命令名]\n§7例如: /help 1 或 /help stop".to_string()
                    }
                };

                // 根据来源选择不同的输出方式
                if is_console {
                    // 控制台：使用日志输出
                    let lines = strip_format_codes(&response);
                    for line in lines.split('\n') {
                        if !line.trim().is_empty() {
                            log::info!("{}", line);
                        }
                    }
                } else {
                    // 玩家：发送聊天消息
                    if let Err(e) = command_data.send_chat_message(&response).await {
                        log::error!("向玩家发送帮助信息失败: {}", e);
                    }
                }

                // 原样转发数据（如果需要的话）
                let _ = send.send(data.data);
                Ok(false)
            }
            ManagerCommand::PlayerClose(uuid) => {
                // 移除心跳任务
                task_map.remove(&uuid.clone());
                let _ = send.send(ManagerCommand::PlayerClose(uuid));
                Ok(false)
            }
            ManagerCommand::TabComplete {
                ref player_uuid,
                ref command_line,
                ref cursor,
                ref suggestions,
            } => {
                let _ = send.send(data.data);
                Ok(false)
            }
        }
    }
}
/// 生成分页帮助信息
fn generate_paginated_help(
    page: usize, 
    is_console: bool, 
    registry: &HashMap<String, crate::message::RegisterCommand>
) -> String {
    // 获取所有命令（去重，避免别名重复显示）
    let mut unique_commands: Vec<&crate::message::RegisterCommand> = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    
    for meta in registry.values() {
        if !seen_names.contains(&meta.name) {
            seen_names.insert(&meta.name);
            unique_commands.push(meta);
        }
    }
    
    // 按字母顺序排序
    unique_commands.sort_by(|a, b| a.name.cmp(&b.name));
    
    let total_commands = unique_commands.len();
    
    // 控制台显示所有命令
    if is_console {
        return generate_console_help(&unique_commands);
    }
    
    // 玩家界面：分页显示（每页10个）
    let page_size = 10;
    let total_pages = (total_commands as f64 / page_size as f64).ceil() as usize;
    
    // 处理页码越界
    let page = if page < 1 { 1 } else if page > total_pages && total_pages > 0 { total_pages } else { page };
    
    let start_idx = (page - 1) * page_size;
    let end_idx = start_idx + page_size;
    
    let mut help_lines = Vec::new();
    
    // 页眉
    help_lines.push(format!("§6===== 帮助 §7(第 {} 页/共 {} 页) §6=====", page, total_pages));
    help_lines.push(format!("§7共有 §e{} §7个可用命令", total_commands));
    help_lines.push("".to_string());
    
    // 显示当前页的命令
    for (i, meta) in unique_commands.iter().enumerate().skip(start_idx).take(page_size) {
        let line_number = i + 1;
        help_lines.push(format!("§a{}. /{} §7- {}", line_number, meta.name, meta.doc));
        
        // 显示别名（如果有）
        if !meta.aliases.is_empty() {
            help_lines.push(format!("§7   别名: §e{}", meta.aliases.join(", ")));
        }
    }
    
    // 页脚：页码导航
    help_lines.push("".to_string());
    
    if total_pages > 1 {
        let navigation = generate_page_navigation(page, total_pages);
        help_lines.push(navigation);
    }
    
    help_lines.push("§7输入 §e/help <命令名> §7查看命令详细用法".to_string());
    help_lines.push("§7输入 §e/help <页码> §7查看指定页的命令列表".to_string());
    
    help_lines.join("\n")
}

/// 生成页码导航
fn generate_page_navigation(current_page: usize, total_pages: usize) -> String {
    let mut navigation_parts = Vec::new();
    
    // 上一页
    if current_page > 1 {
        navigation_parts.push(format!("§e/help {}", current_page - 1));
    } else {
        navigation_parts.push("§8上一页".to_string());
    }
    
    // 页码指示器
    navigation_parts.push(format!("§7[§e{}§7/§e{}§7]", current_page, total_pages));
    
    // 下一页
    if current_page < total_pages {
        navigation_parts.push(format!("§e/help {}", current_page + 1));
    } else {
        navigation_parts.push("§8下一页".to_string());
    }
    
    navigation_parts.join(" §6| ")
}

/// 生成控制台帮助（显示所有命令）
fn generate_console_help(commands: &[&crate::message::RegisterCommand]) -> String {
    let mut help_lines = Vec::new();
    
    help_lines.push("========== 可用命令列表 ==========".to_string());
    help_lines.push("".to_string());
    
    for (i, meta) in commands.iter().enumerate() {
        help_lines.push(format!("{}. /{} - {}", i + 1, meta.name, meta.doc));
        
        // 显示权限和别名
        help_lines.push(format!("   权限: {}", meta.permission));
        
        if !meta.aliases.is_empty() {
            help_lines.push(format!("   别名: {}", meta.aliases.join(", ")));
        }
        
        // 显示参数（如果有）
        if !meta.parameters.is_empty() {
            help_lines.push("   参数:".to_string());
            for param in &meta.parameters {
                let required = if param.required { "必需" } else { "可选" };
                help_lines.push(format!("     - {}: {} ({})", 
                    param.name, param.description, required));
            }
        }
        
        help_lines.push("".to_string());
    }
    
    help_lines.push(format!("共 {} 个命令", commands.len()));
    help_lines.push("使用 /help <命令名> 查看详细用法".to_string());
    
    help_lines.join("\n")
}

/// 生成特定命令的详细帮助（保持原有的函数，但添加分页信息）
fn generate_command_help(cmd_name: &str,
                        registry: &HashMap<String, crate::message::RegisterCommand>) -> String {
    // 查找命令（包括别名）
    let meta = registry.get(cmd_name).or_else(|| {
        // 查找别名
        registry.values()
            .find(|meta| meta.aliases.contains(&cmd_name.to_string()))
    });
    
    match meta {
        Some(meta) => {
            let mut help_lines = Vec::new();
            
            help_lines.push(format!("§6命令: §e/{}", meta.name));
            help_lines.push(format!("§6描述: §7{}", meta.doc));
            help_lines.push(format!("§6权限: §7{}", meta.permission));
            
            if !meta.aliases.is_empty() {
                help_lines.push(format!("§6别名: §7{}", 
                    meta.aliases.join(", ")));
            }
            
            // 参数
            if !meta.parameters.is_empty() {
                help_lines.push("§6参数:".to_string());
                for param in &meta.parameters {
                    let required_mark = if param.required { "§c必需" } else { "§a可选" };
                    let param_type_desc = format_parameter_type(&param.param_type);
                    
                    help_lines.push(format!("  §7{}: {} ({}, {})", 
                        param.name, 
                        param.description,
                        required_mark,
                        param_type_desc
                    ));
                    
                    // 显示参数建议（如果有）
                    if let Some(suggestions) = &param.suggestions {
                        if !suggestions.is_empty() {
                            let suggestions_str = suggestions.iter()
                                .map(|s| format!("§e{}", s))
                                .collect::<Vec<_>>()
                                .join("§7, ");
                            help_lines.push(format!("    建议值: §7{}", suggestions_str));
                        }
                    }
                }
            } else {
                help_lines.push("§6参数: §7无".to_string());
            }
            
            // 用法示例
            help_lines.push("§6用法示例:".to_string());
            let example = generate_command_example(&meta);
            help_lines.push(format!("  §e{}", example));
            
            // 如果是别名，显示原始命令
            if meta.name != cmd_name {
                help_lines.push(format!("§6注意: §7这是 §e/{} §7命令的别名", meta.name));
            }
            
            help_lines.join("\n")
        }
        None => {
            format!("§c没有找到命令 '{}'。\n§7输入 §e/help §7查看所有可用命令。", cmd_name)
        }
    }
}
/// 格式化参数类型描述
fn format_parameter_type(param_type: &ParameterType) -> String {
    match param_type {
        ParameterType::Literal(literal) => format!("字面量: {}", literal),
        ParameterType::String { behavior } => {
            let behavior_desc = match behavior {
                StringBehavior::SingleWord => "单个单词",
                StringBehavior::Quotable => "可引号包裹",
                StringBehavior::Greedy => "贪婪匹配",
            };
            format!("字符串({})", behavior_desc)
        }
        ParameterType::Integer { min, max } => {
            let range_desc = match (min, max) {
                (Some(min_val), Some(max_val)) => format!("{}到{}", min_val, max_val),
                (Some(min_val), None) => format!("≥{}", min_val),
                (None, Some(max_val)) => format!("≤{}", max_val),
                (None, None) => "任意整数".to_string(),
            };
            format!("整数({})", range_desc)
        }
        ParameterType::Boolean => "布尔值".to_string(),
        ParameterType::Player => "玩家名".to_string(),
    }
}

/// 生成命令用法示例
fn generate_command_example(cmd: &RegisterCommand) -> String {
    let mut example_parts = vec![format!("/{}", cmd.name)];
    
    for param in &cmd.parameters {
        let param_example = match &param.param_type {
            ParameterType::Literal(literal) => literal.to_string(),
            ParameterType::String { .. } => match param.name.to_lowercase().as_str() {
                "message" | "text" => "\"消息内容\"",
                "reason" => "\"原因\"",
                "name" | "filename" => "\"名称\"",
                _ => "<文本>",
            }.to_string(),
            ParameterType::Integer { .. } => {
                if param.name.to_lowercase().contains("page") {
                    "1"
                } else if param.name.to_lowercase().contains("time") {
                    "60"
                } else {
                    "123"
                }.to_string()
            }
            ParameterType::Boolean => "true".to_string(),
            ParameterType::Player => "<玩家名>".to_string(),
        };
        
        // 可选参数用方括号包裹
        if param.required {
            example_parts.push(param_example.to_string());
        } else {
            example_parts.push(format!("[{}]", param_example));
        }
    }
    
    example_parts.join(" ")
}
/// 去除Minecraft格式代码（§符号及其后的字符）
fn strip_format_codes(text: &str) -> String {
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