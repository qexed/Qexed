use qexed_config::{
    app::{qexed::Qexed, qexed_auth::Auth, qexed_chat::Chat, qexed_save::Save},
    tool::AppConfigTrait,
};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub qexed: Qexed,
    pub authenticator: qexed_auth::Authenticator,
    pub chat: qexed_chat::ChatService,
    pub save: qexed_save::SaveService,
    pub world: qexed_world::WorldManager,
    pub local_worldgen: Option<std::sync::Arc<qexed_worldgen::WorldGenerator>>,
    pub worldgen_process: Option<std::sync::Arc<crate::worldgen_process::WorldgenProcess>>,
    pub worldgen_client: qexed_world::generator_rpc::VanillaWorldgenClient,
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
    let save = load_config::<Save>("save").await?;
    let save = qexed_save::SaveService::new(save)?;
    save.initialize_directories()?;
    let world = qexed_world::WorldManager::new(save.clone());
    qexed_log::init()?;
    qexed_registry::init()?;
    let local_worldgen = match qexed_worldgen::WorldGenerator::default_cache(0) {
        Ok(generator) => {
            tklog::info!("qexed local worldgen initialized from Mojang cache");
            Some(std::sync::Arc::new(generator))
        }
        Err(err) => {
            tklog::warn!(format!(
                "local worldgen unavailable, Java fallback remains enabled: {err:#}"
            ));
            None
        }
    };
    let (worldgen_process, worldgen_client) = if local_worldgen.is_some() {
        (
            None,
            qexed_world::generator_rpc::VanillaWorldgenClient::new("http://127.0.0.1:1/"),
        )
    } else {
        let worldgen = std::sync::Arc::new(crate::worldgen_process::WorldgenProcess::spawn()?);
        let worldgen_client =
            qexed_world::generator_rpc::VanillaWorldgenClient::new(worldgen.endpoint().to_string());
        tklog::info!(format!(
            "qexed Java worldgen service listening at {}",
            worldgen.endpoint()
        ));
        (Some(worldgen), worldgen_client)
    };
    let config = RuntimeConfig {
        qexed,
        authenticator: qexed_auth::Authenticator::new(auth),
        chat: qexed_chat::ChatService::new(chat)?,
        save,
        world,
        local_worldgen,
        worldgen_process,
        worldgen_client,
    };
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
