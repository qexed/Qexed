include!(concat!(env!("OUT_DIR"), "/generated_log_config.rs"));
use tklog::{LEVEL,ASYNC_LOG, MODE};
use rust_i18n::t;
rust_i18n::i18n!("../../locales");
pub async fn log_init() {
    ASYNC_LOG
        .set_console(true)
        .set_level(LEVEL::Info)
        .set_cutmode_by_time("./logs/server.log", MODE::DAY, 30, true)
        .await
        .set_formatter(&format!("{{level}} [{{time}}]: [{}] {{file}} {{message}}\n",t!("qexed_log.modern.global")));
    module().await;
    ASYNC_LOG.uselog(); //启用官方log库
}
