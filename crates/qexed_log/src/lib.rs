pub async fn log_init(level: LogLevel) {
    let level = level.as_tklog();
    ASYNC_LOG
        .set_console(false)
        .set_level(level)
        .set_cutmode_by_time(
            &format!("./logs/{}.log", t!("qexed_log.modern.global")),
            MODE::DAY,
            30,
            true,
        )
        .await
        .set_formatter(&format!(
            "{{level}} [{{time}}]: [{}] {{file}} {{message}}\n",
            t!("qexed_log.modern.global")
        ))
        .set_custom_handler(log_handler);
    module(level).await;
    ASYNC_LOG.uselog();
}