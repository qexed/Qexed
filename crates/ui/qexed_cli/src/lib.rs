//! Qexed CLI 库
//! 提供基于ratatui的终端界面，只负责显示和转发命令

pub mod app;
pub mod logger;
pub mod ui;

use anyhow::Result;
use chrono::Local;

/// 运行CLI界面
pub async fn run_cli(
    command_tx: tokio::sync::mpsc::Sender<String>,
    log_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    name:&'static str,
) -> Result<()> {
    use crossterm;
    use ratatui;
    // 初始化终端
    let mut terminal = ratatui::init();

    // 设置光标样式
    crossterm::execute!(
        std::io::stdout(),
        crossterm::cursor::SetCursorStyle::BlinkingBar
    )?;

    // 启用鼠标捕获
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

    // 创建CLI应用实例
    let app_ref = app::CliApp::new(command_tx,name);
    // 启动日志监听任务
    if let Some(mut rx) = log_rx {
        let app_ref_clone = app_ref.clone();

        tokio::spawn(async move {
            while let Some(log_message) = rx.recv().await {
                let mut app = app_ref_clone.lock().unwrap();
                if log_message == "::qexed_cli::close" {
                    app.set_exit(true);
                    rx.close();
                    break;
                }
                // 在日志监听任务中添加时间戳
                let now = Local::now();
                let timestamp = now.format("%H:%M:%S").to_string();
                let log_entry = format!("[{}] {}", timestamp, log_message);
                app.logs.push(log_entry);
                if app.logs.len() > app.max_log_lines {
                    app.logs.remove(0);
                }
                // if old_len == 0 || current_scroll == old_len - 1 {
                //     app.log_scroll = app.logs.len().saturating_sub(1);
                // }
            }
        });
    }

    // 运行主循环
    let result = ui::run_cli_loop(&mut terminal, &app_ref).await;

    // 恢复终端设置
    crossterm::execute!(
        std::io::stdout(),
        crossterm::cursor::SetCursorStyle::DefaultUserShape,
        crossterm::event::DisableMouseCapture
    )?;

    // 恢复终端
    ratatui::restore();

    result
}

/// 初始化CLI日志系统
pub fn init_cli_logger(
    log_tx: tokio::sync::mpsc::Sender<String>,
) -> Result<(), log::SetLoggerError> {
    logger::init_cli_logger(log_tx)
}
