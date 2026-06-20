use qexed_config::tool::AppConfigTrait;
use tklog::ASYNC_LOG;
pub fn init() -> anyhow::Result<()> {
    qexed_config::app::qexed_log::Log::load_or_create_default(config_update)?;
    Ok(())
}
async fn config_update(config: qexed_config::app::qexed_log::Log) -> anyhow::Result<()> {
    log_init(config).await;
    Ok(())
}
async fn log_init(config: qexed_config::app::qexed_log::Log) {
    ASYNC_LOG
        .set_level(config.level)
        .set_cutmode_by_time(
            "./logs/qexed.log",
            config.mode,
            config.maxbackups,
            config.compress,
        )
        .await;
    ASYNC_LOG.uselog();
}
