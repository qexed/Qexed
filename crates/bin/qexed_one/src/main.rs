use log;
use qexed_config::{app::qexed_one::One, tool::AppConfigTrait};
use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tklog::{ASYNC_LOG, Format, MODE};
mod api;
mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建日志通道
    let (log_tx, log_rx): (tokio::sync::mpsc::Sender<String>, _) = tokio::sync::mpsc::channel(1000);

    // 创建命令通道
    let (command_tx, mut command_rx): (tokio::sync::mpsc::Sender<String>, _) =
        tokio::sync::mpsc::channel(100);
    // 创建指令通道
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    // 初始化CLI日志系统
    match qexed_cli::init_cli_logger(log_tx.clone()) {
        Ok(_) => {
            // 成功初始化CLI日志
            println!("CLI日志系统初始化成功");
        }
        Err(e) => {
            // CLI日志初始化失败，程序退出
            eprintln!("CLI日志初始化失败: {}", e);
            eprintln!("程序退出");
            return Ok(());
        }
    }

    // 禁用tklog的控制台输出，将日志重定向到CLI
    ASYNC_LOG
        .set_console(false)
        .set_cutmode_by_time("./log/server.log", MODE::DAY, 30, true)
        .await
        .set_format(Format::LevelFlag | Format::Time | Format::ShortFileName)
        .uselog();

    // 启动服务器
    log::info!("读取配置文件中");
    let config: One = One::load_or_create_default()?;
    log::info!("读取配置文件完成");

    log::info!("服务初始化");
    let server = server::Server::init(config).await?;
    log::info!("服务初始化完成");
    log::info!("指令注册中");
    server.register().await?;
    log::info!("指令注册完成");
    // 注册命令 /stop
        // name: String,
        // doc: String,
        // permission: String,
        // parameters: Vec<CommandParameter>,  // 新增参数定义
        // aliases: Vec<String>,  // 新增别名
        // api: Option<UnboundedSender<CommandData>>,
        // success: bool,
    if let qexed_command::message::ManagerCommand::RegisterCommand{success, .. } =
        ReturnMessage::build(qexed_command::message::ManagerCommand::RegisterCommand{
            name:"stop".to_string(),
            doc:"关闭服务器".to_string(),
            permission:"qexed.console.stop".to_string(),
            parameters: vec![],
            aliases:vec![],
            api:Some(cmd_tx),
            success:false,
    })
        .get(&server.api.command)
        .await?
    {
        if success == false {
            log::error!("系统命令/stop注册失败关闭");
            // 这里可以添加服务器关闭逻辑
            let _ = log_tx.send("::qexed_cli::close".to_string()).await;
            return Ok(());
        };
        tokio::spawn(async move {
            // 给任务起个名字，便于日志追踪
            log::debug!("[指令] [stop] 服务启动");

            while let Some(cmd) = cmd_rx.recv().await {
                // 权限验证
                if cmd.is_cmd {
                    // 假设 `is_cmd` 为 true 表示有权限
                    // 尝试发送关闭信号
                    match log_tx.send("::qexed_cli::close".to_string()).await {
                        Ok(_) => {}
                        Err(e) => {}
                    };
                } else {
                    // 无权限，通知玩家
                    match cmd
                        .send_chat_message("§c您没有权限在服务器内关闭服务器。")
                        .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            // 发送聊天消息失败，记录日志但继续运行
                            log::error!("[指令] [stop] 无法向玩家发送无权限提示: {}", e);
                        }
                    };
                    // 权限不足不代表循环要结束，继续监听下一条指令
                    continue;
                }
                break;
            }

            log::debug!("[指令] [stop] 服务关闭");
        });
    } else {
        log::error!("系统命令/stop注册失败关闭");
        // 这里可以添加服务器关闭逻辑
        let _ = log_tx.send("::qexed_cli::close".to_string()).await;
        return Ok(());
    }

    // 启动命令处理任务
    let command_handle = tokio::spawn(async move {
        while let Some(cmd) = command_rx.recv().await {
            // log::info!("收到命令: {}", cmd);
            let _ = ReturnMessage::build(qexed_command::message::ManagerCommand::Command(cmd))
                .get(&server.api.command)
                .await;
            // // 处理服务器命令
            // match cmd.as_str() {
            //     "stop" | "exit" => {
            //         log::info!("收到停止命令，正在关闭服务器...");
            //         // 这里可以添加服务器关闭逻辑
            //         let _ = log_tx.send("::qexed_cli::close".to_string()).await;
            //         return;
            //     }
            //     "reload" => {
            //         log::info!("重新加载配置");
            //     }
            //     "status" => {
            //         log::info!("服务器运行中");
            //     }
            //     "help" => {
            //         log::info!("可用命令:");
            //         log::info!("  stop/exit  - 停止服务器");
            //         log::info!("  reload     - 重新加载配置");
            //         log::info!("  status     - 查看服务器状态");
            //         log::info!("  help       - 显示帮助");
            //     }
            //     _ => {
            //         log::warn!("未知命令: {}", cmd);
            //     }
            // }
        }
    });

    // 启动CLI界面
    log::info!("启动CLI界面...");

    let cli_result = qexed_cli::run_cli(command_tx, Some(log_rx),qexed_config::QEXED_NAME).await;

    // 等待命令处理任务完成
    command_handle.abort(); // 如果CLI退出，就停止命令处理

    match cli_result {
        Ok(_) => {
            log::info!("CLI正常退出");
        }
        Err(e) => {
            log::error!("CLI运行出错: {}", e);
        }
    }

    log::info!("服务器已关闭");
    Ok(())
}
