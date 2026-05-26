use std::sync::Arc;

use crate::auth::Authenticator;

#[derive(Clone)]
pub struct ServerContext {
    pub config: Arc<qexed_config::app::qexed::Qexed>,
    pub authenticator: Arc<Authenticator>,
    pub world: Arc<crate::world::WorldManager>,
    pub players: Arc<crate::players::PlayerManager>,
    pub entities: Arc<crate::entities::EntityManager>,
    pub player_data: Arc<crate::player_data::PlayerDataManager>,
    pub permissions: Arc<crate::permissions::PermissionManager>,
    pub plugins: Arc<crate::plugins::PluginManager>,
    pub content_filter: Arc<crate::content_filter::ContentFilter>,
    pub resource_pack: Arc<crate::resource_pack::ResourcePackManager>,
    pub code_of_conducts: Arc<crate::code_of_conduct::CodeOfConductTexts>,
}

impl ServerContext {
    pub async fn new(config: qexed_config::app::qexed::Qexed) -> anyhow::Result<Self> {
        Self::new_with_code_of_conduct_dir(
            config,
            crate::code_of_conduct::DEFAULT_CODE_OF_CONDUCT_DIR,
        )
        .await
    }

    pub async fn new_with_code_of_conduct_dir(
        config: qexed_config::app::qexed::Qexed,
        code_of_conduct_dir: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<Self> {
        let plugins = Arc::new(crate::plugins::PluginManager::load_default());
        plugins.emit_init();
        plugins.emit_config_reload("config/qexed.toml");
        plugins.emit_language_change(&config.language);

        let world_generator = crate::world::generator::from_config(&config.server.world);
        let world = crate::world::WorldManager::with_generator(
            config.server.world.path.clone(),
            crate::world::WorldLightMode::from(&config.server.world.light),
            crate::world::WorldLightAlgorithm::from(&config.server.world.light_algorithm),
            crate::world::light_gpu_from_config(&config.server.world.gpu),
            config.server.world.read_only,
            world_generator,
        );
        world.ensure_storage(&config.server.world.dimension)?;
        let player_data = crate::player_data::PlayerDataManager::from_config(
            config.server.world.path.clone(),
            &config.server.player_data,
        )
        .await?;
        let permissions =
            crate::permissions::PermissionManager::from_config(&config.server.permissions).await?;
        let content_filter =
            crate::content_filter::ContentFilter::from_config(&config.server.content_filter)?;
        let entity_ids = Arc::new(crate::entities::EntityIdAllocator::default());
        let entities = crate::entities::EntityManager::from_config(
            &config.server.entities,
            entity_ids.clone(),
        )?;
        let mut resource_pack =
            crate::resource_pack::ResourcePackManager::from_config(&config.server.resource_pack)
                .await?;
        resource_pack.start().await?;
        let code_of_conducts = crate::code_of_conduct::CodeOfConductTexts::load(
            config.server.code_of_conduct,
            code_of_conduct_dir,
        )?;
        Ok(Self {
            config: Arc::new(config),
            authenticator: Arc::new(Authenticator::new()?),
            world: Arc::new(world),
            players: Arc::new(crate::players::PlayerManager::new(entity_ids)),
            entities: Arc::new(entities),
            player_data: Arc::new(player_data),
            permissions: Arc::new(permissions),
            plugins,
            content_filter: Arc::new(content_filter),
            resource_pack: Arc::new(resource_pack),
            code_of_conducts: Arc::new(code_of_conducts),
        })
    }
}
