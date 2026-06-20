use qexed_config::{app::qexed::Qexed, tool::AppConfigTrait};

pub async fn load(
    args: &qexed_config::app::qexed::args::ServerArgs,
) -> anyhow::Result<Option<Qexed>> {
    let config_path = args
        .config_path
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("./config"));
    qexed_config::CONFIG_PATH
        .set(config_path)
        .map_err(|_| anyhow::anyhow!("CONFIG_PATH is already initialized"))?;

    let plugin_path = args
        .plugin_path
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("./plugin"));
    qexed_plugin::PLUGINS_PATH
        .set(plugin_path)
        .map_err(|_| anyhow::anyhow!("PLUGINS_PATH is already initialized"))?;

    let config = load_qexed_config().await?;
    qexed_log::init()?;
    qexed_registry::init()?;
    qexed_plugin::init().await?;
    qexed_plugin::plugin_manager()
        .fire(&qexed_plugin_api::PluginLoadFinishEvent)
        .await;

    Ok(Some(config))
}

async fn load_qexed_config() -> anyhow::Result<Qexed> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let join_handle = Qexed::load_or_create_default(move |config| async move {
        sender
            .send(config)
            .map_err(|_| anyhow::anyhow!("failed to return qexed config"))?;
        Ok(())
    })?;

    join_handle.await??;
    receiver
        .await
        .map_err(|_| anyhow::anyhow!("qexed config loader dropped before returning config"))
}
