// 在 logger.rs 中
use log::{Log, Metadata, Record, SetLoggerError};
use std::sync::{ Mutex};
use once_cell::sync::Lazy;

/// 全局日志通道发送器
static LOG_SENDER: Lazy<Mutex<Option<tokio::sync::mpsc::Sender<String>>>> = Lazy::new(|| {
    Mutex::new(None)
});

/// 自定义日志记录器
pub struct ChannelLogger;

impl ChannelLogger {
    /// 创建新的ChannelLogger
    pub const fn new() -> Self {
        Self
    }
    
    /// 设置全局日志发送器
    pub fn set_global_sender(sender: tokio::sync::mpsc::Sender<String>) {
        let mut sender_guard = LOG_SENDER.lock().unwrap();
        *sender_guard = Some(sender);
    }
}

impl Log for ChannelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        // 格式化日志消息
        let message = format!(
            "[{}] {}",
            record.level(),
            record.args()
        );
        
        // 尝试从全局获取日志发送器
        if let Ok(sender_guard) = LOG_SENDER.lock() {
            if let Some(tx) = &*sender_guard {
                let _ = tx.try_send(message);
            }
        }
    }

    fn flush(&self) {}
}

/// 静态的 ChannelLogger 实例
static CHANNEL_LOGGER: ChannelLogger = ChannelLogger::new();

/// 初始化CLI日志系统
pub fn init_cli_logger(log_tx: tokio::sync::mpsc::Sender<String>) -> Result<(), SetLoggerError> {
    ChannelLogger::set_global_sender(log_tx);
    log::set_max_level(log::LevelFilter::Trace);
    
    // 使用静态实例
    log::set_logger(&CHANNEL_LOGGER)
}