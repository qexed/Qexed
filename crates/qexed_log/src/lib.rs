include!(concat!(env!("OUT_DIR"), "/generated_log_config.rs"));
use tklog::{LEVEL,ASYNC_LOG, MODE};
pub async fn log_init() {
    ASYNC_LOG
        .set_console(true)
        .set_level(LEVEL::Info)
        .set_cutmode_by_time("./logs/server.log", MODE::DAY, 30, true)
        .await
        .set_formatter("[Global] {level} {time} {file}:{message}\n");
    module().await;
    ASYNC_LOG.uselog(); //启用官方log库
    log::info!("日志系统初始化完成");
        
}
