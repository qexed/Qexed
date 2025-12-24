pub async fn run(
    config: qexed_config::app::qexed_rule::RuleConfig,
) -> anyhow::Result<qexed_shared::Shared<qexed_config::app::qexed_rule::RuleConfig>> {
    let app = qexed_shared::Shared::new(config).await?;
    log::info!("[服务] 规则管理 已启用");
    Ok(app)
}