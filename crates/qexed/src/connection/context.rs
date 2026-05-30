use std::sync::{Arc, OnceLock};

use crate::auth::Authenticator;

#[derive(Clone)]
pub struct ServerContext {
    pub config: Arc<crate::config::RuntimeConfig>,
    pub authenticator: Arc<Authenticator>,
    pub world: Arc<crate::world::WorldManager>,
    pub world_rules: Arc<crate::world::WorldRulesManager>,
    pub players: Arc<crate::players::PlayerManager>,
    pub entities: Arc<crate::entities::EntityManager>,
    pub player_data: Arc<crate::player_data::PlayerDataManager>,
    pub permissions: Arc<crate::permissions::PermissionManager>,
    pub plugins: Arc<crate::plugins::PluginManager>,
    pub player_audit: Arc<crate::audit::PlayerAuditLogger>,
    pub content_filter: Arc<crate::content_filter::ContentFilter>,
    pub resource_pack: Arc<crate::resource_pack::ResourcePackManager>,
    pub code_of_conducts: Arc<crate::code_of_conduct::CodeOfConductTexts>,
    plugin_startup_applied: Arc<OnceLock<()>>,
}

impl ServerContext {
    pub async fn new(config: impl Into<crate::config::RuntimeConfig>) -> anyhow::Result<Self> {
        Self::new_with_code_of_conduct_dir(
            config,
            crate::code_of_conduct::DEFAULT_CODE_OF_CONDUCT_DIR,
        )
        .await
    }

    pub async fn new_with_code_of_conduct_dir(
        config: impl Into<crate::config::RuntimeConfig>,
        code_of_conduct_dir: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<Self> {
        let config = config.into();
        let plugins = Arc::new(crate::plugins::PluginManager::load_default());

        let world_generator = crate::world::generator::from_config(&config.world);
        let world_rules = crate::world::WorldRulesManager::from_world_config(&config.world)?;
        let world = crate::world::WorldManager::with_generator(
            config.world.path.clone(),
            crate::world::WorldLightMode::from(&config.world.light),
            crate::world::WorldLightAlgorithm::from(&config.world.light_algorithm),
            crate::world::light_gpu_from_config(&config.world.gpu),
            config.world.read_only,
            world_generator,
        )
        .with_worlds(&config.world.worlds)
        .with_instances(&config.world.instances)
        .with_precompiled_chunks(crate::world::PrecompiledChunkSettings::from(
            &config.world.precompiled_chunks,
        ));
        world.ensure_configured_storage(&config.world)?;
        plugins.set_pathfinding_service(std::sync::Arc::new(ServerPathfindingService {
            world: std::sync::Arc::new(world.clone()),
        }));
        let player_data = crate::player_data::PlayerDataManager::from_config(
            config.world.path.clone(),
            &config.server.player_data,
        )
        .await?;
        let permissions =
            crate::permissions::PermissionManager::from_config(&config.server.permissions).await?;
        let player_audit =
            crate::audit::PlayerAuditLogger::from_config(&config.server.player_audit);
        let content_filter =
            crate::content_filter::ContentFilter::from_config(&config.server.content_filter)?;
        let entity_ids = Arc::new(crate::entities::EntityIdAllocator::default());
        let players = Arc::new(crate::players::PlayerManager::new(entity_ids.clone()));
        let entities = crate::entities::EntityManager::from_config_with_skin_lookup(
            &config.server.entities,
            entity_ids.clone(),
        )
        .await?;
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
            world_rules: Arc::new(world_rules),
            players,
            entities: Arc::new(entities),
            player_data: Arc::new(player_data),
            permissions: Arc::new(permissions),
            plugins,
            player_audit: Arc::new(player_audit),
            content_filter: Arc::new(content_filter),
            resource_pack: Arc::new(resource_pack),
            code_of_conducts: Arc::new(code_of_conducts),
            plugin_startup_applied: Arc::new(OnceLock::new()),
        })
    }

    pub fn ensure_plugins_initialized(&self) {
        self.plugin_startup_applied.get_or_init(|| {
            self.plugins.ensure_initialized(&self.config.language);
            apply_plugin_npc_mutations(
                &self.config.server.entity_rendering,
                &self.plugins,
                &self.players,
                &self.entities,
            );
        });
    }
}

#[derive(Debug)]
struct ServerPathfindingService {
    world: std::sync::Arc<crate::world::WorldManager>,
}

impl crate::plugins::host::PathfindingService for ServerPathfindingService {
    fn find_path(&self, query: &str) -> Option<String> {
        let mut parts = query.split_whitespace();
        let dimension = parts.next()?;
        let start = qexed_packet::net_types::Position {
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
            z: parts.next()?.parse().ok()?,
        };
        let goal = qexed_packet::net_types::Position {
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
            z: parts.next()?.parse().ok()?,
        };
        let max_nodes = parts
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(4096);
        let path = crate::play::pathfinding::find_path(crate::play::pathfinding::PathQuery {
            world: &self.world,
            dimension,
            start,
            goal,
            max_nodes,
        })?;
        Some(
            path.into_iter()
                .map(|position| format!("{},{},{}", position.x, position.y, position.z))
                .collect::<Vec<_>>()
                .join(";"),
        )
    }
}

fn apply_plugin_npc_mutations(
    rendering: &qexed_config::app::qexed::server::EntityRendering,
    plugins: &crate::plugins::PluginManager,
    players: &crate::players::PlayerManager,
    entities: &crate::entities::EntityManager,
) {
    for operation in plugins.query_npc_mutations("startup") {
        match operation {
            crate::plugins::NpcMutationOp::Upsert { npc } => {
                let key = npc.key.trim();
                if key.is_empty() {
                    continue;
                }

                if entities.entity_by_key(key).is_some() {
                    let _ = entities.remove(players, key);
                }
                let name = if npc.name.trim().is_empty() {
                    key.to_string()
                } else {
                    npc.name.clone()
                };
                let display_name = if npc.display_name.trim().is_empty() {
                    name.clone()
                } else {
                    npc.display_name.clone()
                };
                let entity = crate::entities::EntitySpawnRequest {
                    key: key.to_string(),
                    kind: crate::entities::ManagedEntityKind::Npc,
                    entity_type: "minecraft:player".to_string(),
                    dimension: npc.dimension.clone(),
                    position: qexed_protocol::to_client::play::add_entity::EntityPosition {
                        x: npc.x,
                        y: npc.y,
                        z: npc.z,
                        yaw: npc.yaw,
                        pitch: npc.pitch,
                        on_ground: false,
                    },
                    name,
                    display_name,
                    skin_textures: npc.skin_textures,
                    skin_signature: npc.skin_signature,
                    data: 0,
                    look_at_players: npc.look_at_players,
                    main_hand_event: npc.main_hand_event,
                    off_hand_event: npc.off_hand_event,
                    attack_event: npc.attack_event,
                };
                let spawned = match entities.spawn_local(entity) {
                    Ok(entity) => entity,
                    Err(err) => {
                        log::warn!("plugin npc upsert failed: key={key}, error={err:#}");
                        continue;
                    }
                };
                if let Err(err) =
                    entities.send_spawn_to_rendered_viewers(players, rendering, &spawned)
                {
                    log::warn!("plugin npc spawn packet failed: key={key}, error={err:#}");
                } else {
                    log::info!(
                        "plugin npc upserted: key={key}, dimension={}, x={}, y={}, z={}",
                        npc.dimension,
                        npc.x,
                        npc.y,
                        npc.z
                    );
                }
            }
            crate::plugins::NpcMutationOp::Remove { key } => {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                if entities.entity_by_key(key).is_none() {
                    continue;
                }
                match entities.remove_local(key) {
                    Ok(entity) => {
                        if let Err(err) =
                            entities.send_remove_to_rendered_viewers(players, rendering, &entity)
                        {
                            log::warn!("plugin npc remove packet failed: key={key}, error={err:#}");
                        }
                    }
                    Err(err) => {
                        log::warn!("plugin npc remove failed: key={key}, error={err:#}");
                    }
                }
            }
        }
    }
}
