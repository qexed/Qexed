use qexed_config::{
    app::{qexed::Qexed, qexed_auth::Auth, qexed_chat::Chat},
    tool::AppConfigTrait,
};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub qexed: Qexed,
    pub authenticator: qexed_auth::Authenticator,
    pub chat: qexed_chat::ChatService,
}

pub async fn load(
    args: &qexed_config::app::qexed::args::ServerArgs,
) -> anyhow::Result<Option<RuntimeConfig>> {
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

    let qexed = load_config::<Qexed>("qexed").await?;
    let auth = load_config::<Auth>("auth").await?;
    let chat = load_config::<Chat>("chat").await?;
    let config = RuntimeConfig {
        qexed,
        authenticator: qexed_auth::Authenticator::new(auth),
        chat: qexed_chat::ChatService::new(chat)?,
    };
    qexed_log::init()?;
    qexed_registry::init()?;
    qexed_plugin::init().await?;
    qexed_plugin::plugin_manager()
        .fire(&qexed_plugin_api::PluginLoadFinishEvent)
        .await;

    Ok(Some(config))
}

async fn load_config<T>(name: &'static str) -> anyhow::Result<T>
where
    T: AppConfigTrait + Send + 'static,
{
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let join_handle = T::load_or_create_default(move |config| async move {
        sender
            .send(config)
            .map_err(|_| anyhow::anyhow!("failed to return {name} config"))?;
        Ok(())
    })?;

    join_handle.await??;
    receiver
        .await
        .map_err(|_| anyhow::anyhow!("{name} config loader dropped before returning config"))
}
