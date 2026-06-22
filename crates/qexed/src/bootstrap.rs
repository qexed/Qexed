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
    pub entities: RuntimeEntities,
    pub local_worldgen: Option<std::sync::Arc<qexed_worldgen::WorldGenerator>>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeEntities {
    store: std::sync::Arc<std::sync::Mutex<qexed_entity::EntityStore>>,
}

impl RuntimeConfig {
    pub fn reload_save_system(&mut self) -> anyhow::Result<()> {
        self.world.flush_writes();
        let save = reload_save_service()?;
        self.save = save.clone();
        self.world = qexed_world::WorldManager::new(save);
        Ok(())
    }
}

impl RuntimeEntities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn_player(
        &self,
        uuid: uuid::Uuid,
        pose: qexed_entity::EntityPose,
    ) -> qexed_entity::EntitySnapshot {
        let mut store = self.store.lock().expect("entity store mutex poisoned");
        let id = store.spawn_with_uuid(uuid, qexed_entity::EntityKind::Player, pose);
        store
            .get(id)
            .expect("spawned player entity must be readable")
            .snapshot()
    }

    pub fn update_pose(&self, id: qexed_entity::EntityId, pose: qexed_entity::EntityPose) -> bool {
        self.store
            .lock()
            .expect("entity store mutex poisoned")
            .update_pose(id, pose)
    }

    pub fn despawn(&self, id: qexed_entity::EntityId) -> Option<qexed_entity::Entity> {
        self.store
            .lock()
            .expect("entity store mutex poisoned")
            .despawn(id)
    }
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
    let save = reload_save_service()?;
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
                "local worldgen unavailable; missing chunks will fall back to empty chunks: {err:#}"
            ));
            None
        }
    };
    let config = RuntimeConfig {
        qexed,
        authenticator: qexed_auth::Authenticator::new(auth),
        chat: qexed_chat::ChatService::new(chat)?,
        save,
        world,
        entities: RuntimeEntities::new(),
        local_worldgen,
    };
    qexed_plugin::init().await?;
    qexed_plugin::plugin_manager()
        .fire(&qexed_plugin_api::PluginLoadFinishEvent)
        .await;

    Ok(Some(config))
}

fn reload_save_service() -> anyhow::Result<qexed_save::SaveService> {
    let save = Save::reload_from_disk()?;
    let save = qexed_save::SaveService::new(save)?;
    save.initialize_directories()?;
    Ok(save)
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
