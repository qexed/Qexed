use std::sync::OnceLock;
use std::time::SystemTime;
use tokio::sync::mpsc::UnboundedSender;
use qexed_command::message::ManagerCommand as CommandManagerCommand;
use qexed_task::message::return_message::ReturnMessage;

// 版本信息结构体
#[derive(Debug, Clone)]
pub struct ServerVersionInfo {
    pub server_name: String,
    pub minecraft_version: String,
    pub build_number: String,
    pub build_time: String,
    pub api_version: String,
    pub qexed_version: String,
    pub startup_time: SystemTime,
}

impl ServerVersionInfo {
    pub fn new() -> Self {
        let now = SystemTime::now();
        
        // 使用 chrono 获取构建时间
        let build_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        
        Self {
            server_name: qexed_config::QEXED_NAME.to_string(),
            minecraft_version: qexed_config::MC_VERSION.to_string(),
            build_number: "a?".to_string(),
            build_time,
            api_version: "?".to_string(),
            qexed_version: qexed_config::QEXED_VERSION.to_string(),
            startup_time: now,
        }
    }
    
    pub fn format_full_version(&self) -> String {
        format!(
            "This server is running {} version {}-{} (Implementing API version {})\n\
             * Qexed version: {}\n\
             * Build time: {}\n\
             * Uptime: {}",
            self.server_name,
            self.minecraft_version,
            self.build_number,
            self.api_version,
            self.qexed_version,
            self.build_time,
            self.format_uptime()
        )
    }
    
    pub fn format_short_version(&self) -> String {
        format!(
            "{} {}-{} (Qexed {})",
            self.server_name,
            self.minecraft_version,
            self.build_number,
            self.qexed_version
        )
    }
    
    pub fn format_uptime(&self) -> String {
        match self.startup_time.elapsed() {
            Ok(duration) => {
                let days = duration.as_secs() / 86400;
                let hours = (duration.as_secs() % 86400) / 3600;
                let minutes = (duration.as_secs() % 3600) / 60;
                let seconds = duration.as_secs() % 60;
                
                if days > 0 {
                    format!("{}天{}小时{}分{}秒", days, hours, minutes, seconds)
                } else if hours > 0 {
                    format!("{}小时{}分{}秒", hours, minutes, seconds)
                } else if minutes > 0 {
                    format!("{}分{}秒", minutes, seconds)
                } else {
                    format!("{}秒", seconds)
                }
            }
            Err(_) => "未知".to_string(),
        }
    }
}

// 使用 OnceLock 替代 lazy_static
static SERVER_VERSION: OnceLock<ServerVersionInfo> = OnceLock::new();

pub fn get_server_version() -> &'static ServerVersionInfo {
    SERVER_VERSION.get_or_init(ServerVersionInfo::new)
}

pub async fn register_version_command(
    command_api: &UnboundedSender<ReturnMessage<CommandManagerCommand>>,
) -> anyhow::Result<()> {
    qexed_command::register::register_command(
        "version",
        "查看服务器版本信息",
        "qexed.console.version",
        vec![
            qexed_command::message::CommandParameter {
                name: "verbose".to_string(),
                description: "显示详细版本信息".to_string(),
                required: false,
                param_type: qexed_command::message::ParameterType::String {
                    behavior: qexed_command::message::StringBehavior::SingleWord,
                },
                suggestions: Some(vec!["full".to_string(), "short".to_string()]),
            },
        ],
        vec!["ver", "about"],
        command_api,
        move |mut cmd_rx| {
            async move {
                while let Some(cmd) = cmd_rx.recv().await {
                    // 解析命令参数
                    let args: Vec<&str> = cmd.command_line.split_whitespace().collect();
                    
                    // 跳过命令名
                    let mut args_iter = args.iter().skip(1);
                    let mut verbose = false;
                    let mut show_short = false;
                    
                    // 解析参数
                    while let Some(arg) = args_iter.next() {
                        match *arg {
                            "full" | "--verbose" | "-v" => verbose = true,
                            "short" | "-s" => show_short = true,
                            _ => {
                                // 忽略未知参数
                            }
                        }
                    }
                    
                    // 根据参数选择输出格式
                    let version_info = get_server_version();
                    let version_message = if show_short {
                        version_info.format_short_version()
                    } else if verbose {
                        version_info.format_full_version()
                    } else {
                        format!(
                            "This server is running {} version {}-{} (Implementing API version {})",
                            version_info.server_name,
                            version_info.minecraft_version,
                            version_info.build_number,
                            version_info.api_version
                        )
                    };
                    cmd.send_chat_message(&version_message).await?;
                    
                    // 如果未指定详细模式，添加提示
                    if !verbose && !show_short {
                        let hint_message = "§7使用 §f/version full §7查看详细版本信息";
                        cmd.send_chat_message(&hint_message).await?;
                    }
                }
                
                Ok(())
            }
        },
    )
    .await
}

// 扩展：用于在服务器启动时记录版本信息
pub fn log_startup_version() {
    let version = get_server_version();
    log::info!("Starting Qexed version {}", version.qexed_version);
    log::info!("Minecraft version: {}", version.minecraft_version);
    log::info!("API version: {}", version.api_version);
    log::info!("Build time: {}", version.build_time);
}

// 扩展：用于API获取版本信息
pub fn get_version_info() -> &'static ServerVersionInfo {
    get_server_version()
}

// 扩展：检查是否是最新版本（占位符）
pub fn check_update_status() -> String {
    "* You are running the latest version".to_string()
}