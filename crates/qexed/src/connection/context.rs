use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::auth::Authenticator;

#[derive(Clone)]
pub struct ServerContext {
    pub config: Arc<crate::config::RuntimeConfig>,
    pub authenticator: Arc<Authenticator>,
    pub world: Arc<crate::world::WorldManager>,
    pub world_rules: Arc<crate::world::WorldRulesManager>,
    pub ore_pits: Arc<crate::world::OrePitManager>,
    pub cluster_entities: Option<Arc<crate::cluster_entities::ClusterEntityController>>,
    pub players: Arc<crate::players::PlayerManager>,
    pub entities: Arc<crate::entities::EntityManager>,
    pub player_data: Arc<crate::player_data::PlayerDataManager>,
    pub permissions: Arc<crate::permissions::PermissionManager>,
    pub plugins: Arc<crate::plugins::PluginManager>,
    pub profiler: Arc<qexed_profiler::Profiler>,
    pub player_audit: Arc<crate::audit::PlayerAuditLogger>,
    pub content_filter: Arc<crate::content_filter::ContentFilter>,
    pub resource_pack: Arc<crate::resource_pack::ResourcePackManager>,
    pub code_of_conducts: Arc<crate::code_of_conduct::CodeOfConductTexts>,
    pub warden: Arc<crate::warden::WardenManager>,
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
        plugins.configure_economy(&config.server.economy);
        plugins.configure_structured_storage(&config.server.plugin_storage);

        let local_world_generator = crate::world::generator::from_config(&config.world);
        let world_generator = crate::world::ClusteredWorldGenerator::from_config(
            &config.world.cluster,
            local_world_generator.clone(),
        )
        .map(|generator| std::sync::Arc::new(generator) as _)
        .unwrap_or_else(|| {
            if config.world.cluster.enable {
                log::warn!(
                    "world cluster is enabled but quadrant shard config is incomplete; using local world generator"
                );
            }
            local_world_generator
        });
        let world_rules = crate::world::WorldRulesManager::from_world_config(&config.world)?;
        let world = crate::world::WorldManager::with_generator(
            config.world.path.clone(),
            crate::world::WorldLightMode::from(&config.world.light),
            crate::world::WorldLightAlgorithm::from(&config.world.light_algorithm),
            config.world.read_only,
            world_generator,
        )
        .with_worlds(&config.world.worlds)
        .with_instances(&config.world.instances)
        .with_edit_regions(&config.world.edit_regions)
        .with_precompiled_chunks(crate::world::PrecompiledChunkSettings::from(
            &config.world.precompiled_chunks,
        ));
        world.ensure_configured_storage(&config.world)?;
        let ore_pits = crate::world::OrePitManager::from_config(&config.world.ore_pits);
        let cluster_entities =
            crate::cluster_entities::ClusterEntityController::from_config(&config.world.cluster)
                .map(Arc::new);
        plugins.set_pathfinding_service(Arc::new(ServerPathfindingService {
            world: Arc::new(world.clone()),
            cache: Mutex::new(PathfindingCache::default()),
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
        let warden = crate::warden::WardenManager::from_config(config.warden.clone());
        let entity_ids = Arc::new(crate::entities::EntityIdAllocator::default());
        let players = Arc::new(crate::players::PlayerManager::new(entity_ids.clone()));
        plugins.set_world_edit_service(Arc::new(ServerWorldEditService {
            world: Arc::new(world.clone()),
            players: players.clone(),
        }));
        let gateway_entities = if cluster_entities.is_some() {
            qexed_config::app::qexed::server::Entities::default()
        } else {
            config.server.entities.clone()
        };
        let entities = Arc::new(
            crate::entities::EntityManager::from_config_with_skin_lookup(
                &gateway_entities,
                entity_ids.clone(),
            )
            .await?,
        );
        plugins.set_entity_control_service(Arc::new(ServerEntityControlService {
            entities: entities.clone(),
            players: players.clone(),
            rendering: config.server.entity_rendering.clone(),
            entity_config: gateway_entities.clone(),
        }));
        let mut resource_pack =
            crate::resource_pack::ResourcePackManager::from_config(&config.server.resource_pack)
                .await?;
        resource_pack.start().await?;
        let code_of_conducts = crate::code_of_conduct::CodeOfConductTexts::load(
            config.server.code_of_conduct,
            code_of_conduct_dir,
        )?;
        let profiler = Arc::new(qexed_profiler::Profiler::new());
        crate::profiler::init(profiler.as_ref().clone());
        Ok(Self {
            config: Arc::new(config),
            authenticator: Arc::new(Authenticator::new()?),
            world: Arc::new(world),
            world_rules: Arc::new(world_rules),
            ore_pits: Arc::new(ore_pits),
            cluster_entities,
            players,
            entities,
            player_data: Arc::new(player_data),
            permissions: Arc::new(permissions),
            plugins,
            profiler,
            player_audit: Arc::new(player_audit),
            content_filter: Arc::new(content_filter),
            resource_pack: Arc::new(resource_pack),
            code_of_conducts: Arc::new(code_of_conducts),
            warden: Arc::new(warden),
            plugin_startup_applied: Arc::new(OnceLock::new()),
        })
    }

    pub fn ensure_plugins_initialized(&self) {
        self.plugin_startup_applied.get_or_init(|| {
            self.plugins.ensure_initialized(&self.config.language);
            self.entities
                .register_custom_entities(self.plugins.custom_entities());
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
    world: Arc<crate::world::WorldManager>,
    cache: Mutex<PathfindingCache>,
}

impl crate::plugins::host::PathfindingService for ServerPathfindingService {
    fn find_path(&self, query: &str) -> Option<String> {
        let query = ParsedPathQuery::parse(query)?;
        let key = PathfindingCacheKey::new(self.world.cache_epoch(), &query);
        let now = Instant::now();
        if let Some(cached) = self
            .cache
            .lock()
            .expect("pathfinding cache poisoned")
            .get(&key, now)
        {
            return cached;
        }

        let path = crate::play::pathfinding::find_path(crate::play::pathfinding::PathQuery {
            world: &self.world,
            dimension: &query.dimension,
            start: query.start,
            goal: query.goal,
            max_nodes: query.max_nodes,
            cached_only: false,
        })
        .map(|path| {
            path.into_iter()
                .map(|position| format!("{},{},{}", position.x, position.y, position.z))
                .collect::<Vec<_>>()
                .join(";")
        });
        self.cache
            .lock()
            .expect("pathfinding cache poisoned")
            .insert(key, path.clone(), Instant::now());
        path
    }
}

#[derive(Debug)]
struct ServerWorldEditService {
    world: Arc<crate::world::WorldManager>,
    players: Arc<crate::players::PlayerManager>,
}

impl crate::plugins::host::WorldEditService for ServerWorldEditService {
    fn set_block(&self, query: &str) -> i32 {
        let Some(request) = ParsedWorldEditQuery::parse_set(query) else {
            return -1;
        };
        let Some(region) = self
            .world
            .editable_region_for_plugin_write(&request.dimension, &request.position)
        else {
            return 1;
        };
        self.write_block(
            &request.dimension,
            request.position.clone(),
            request.block_state,
            region.runtime_only,
        );
        self.broadcast_change(&request.dimension, request.position, request.block_state);
        0
    }

    fn set_blocks(&self, query: &str) -> i32 {
        let mut requests = Vec::new();
        for line in query.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let Some(request) = ParsedWorldEditQuery::parse_set(line) else {
                return -1;
            };
            let Some(region) = self
                .world
                .editable_region_for_plugin_write(&request.dimension, &request.position)
            else {
                return 1;
            };
            requests.push((request, region.runtime_only));
        }

        for (request, runtime_only) in requests {
            self.write_block(
                &request.dimension,
                request.position.clone(),
                request.block_state,
                runtime_only,
            );
            self.broadcast_change(&request.dimension, request.position, request.block_state);
        }
        0
    }

    fn break_block(&self, query: &str) -> i32 {
        let Some(request) = ParsedWorldEditQuery::parse_break(query) else {
            return -1;
        };
        let Some(region) = self
            .world
            .editable_region_for_plugin_write(&request.dimension, &request.position)
        else {
            return 1;
        };
        let air = crate::inventory::air_block_state();
        self.write_block(
            &request.dimension,
            request.position.clone(),
            air,
            region.runtime_only,
        );
        self.broadcast_change(&request.dimension, request.position, air);
        0
    }

    fn register_region(&self, query: &str) -> i32 {
        let Some(region) = ParsedWorldEditRegion::parse(query) else {
            return -1;
        };
        self.world.register_edit_region(region);
        0
    }
}

impl ServerWorldEditService {
    fn write_block(
        &self,
        dimension: &str,
        position: qexed_packet::net_types::Position,
        block_state: i32,
        runtime_only: bool,
    ) {
        if runtime_only || self.world.read_only() {
            self.world
                .set_runtime_block(dimension, position, block_state);
        } else {
            self.world.place_block(dimension, position, block_state);
        }
    }

    fn broadcast_change(
        &self,
        dimension: &str,
        position: qexed_packet::net_types::Position,
        block_state: i32,
    ) {
        let light_update = if self.world.dynamic_light_enabled() {
            let update = self.world.light_update(
                dimension,
                position.x.div_euclid(16),
                position.z.div_euclid(16),
            );
            qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(update).ok()
        } else {
            None
        };
        self.players.broadcast_block_changed(
            uuid::Uuid::nil(),
            dimension,
            position,
            block_state,
            light_update,
        );
    }
}

#[derive(Debug)]
struct ServerEntityControlService {
    entities: Arc<crate::entities::EntityManager>,
    players: Arc<crate::players::PlayerManager>,
    rendering: qexed_config::app::qexed::server::EntityRendering,
    entity_config: qexed_config::app::qexed::server::Entities,
}

impl crate::plugins::host::EntityControlService for ServerEntityControlService {
    fn upsert(&self, plugin_name: &str, query: &str) -> i32 {
        let Some(mut request) = ParsedEntityUpsert::parse(plugin_name, query) else {
            return -1;
        };
        if crate::entities::EntityManager::entity_type_disabled(
            &self.entity_config,
            &request.entity_type,
        ) {
            if let Some(existing) = self.entities.entity_by_key(&request.key) {
                if let Err(err) = self.entities.send_remove_to_rendered_viewers(
                    &self.players,
                    &self.rendering,
                    &existing,
                ) {
                    log::warn!(
                        "disabled plugin entity remove packet failed: key={}, error={err:#}",
                        request.key
                    );
                }
                if let Err(err) = self.entities.remove_local(&request.key) {
                    log::warn!(
                        "disabled plugin entity remove failed: key={}, error={err:#}",
                        request.key
                    );
                    return -1;
                }
            }
            return 1;
        }
        crate::entities::EntityManager::apply_entity_ai_overrides(
            &self.entity_config,
            &request.entity_type,
            &mut request.ai,
            &mut request.ai_params,
            &mut request.auto_jump,
        );
        if let Some(existing) = self.entities.entity_by_key(&request.key) {
            if let Err(err) = self.entities.send_remove_to_rendered_viewers(
                &self.players,
                &self.rendering,
                &existing,
            ) {
                log::warn!(
                    "plugin entity replacement remove packet failed: key={}, error={err:#}",
                    request.key
                );
            }
            if let Err(err) = self.entities.remove_local(&request.key) {
                log::warn!(
                    "plugin entity replacement remove failed: key={}, error={err:#}",
                    request.key
                );
                return -1;
            }
        }

        let spawn = crate::entities::EntitySpawnRequest {
            key: request.key.clone(),
            kind: crate::entities::ManagedEntityKind::Entity,
            entity_type: request.entity_type,
            entity_type_id_override: None,
            dimension: request.dimension,
            position: request.position,
            name: request.name,
            display_name: request.display_name,
            skin_textures: String::new(),
            skin_signature: String::new(),
            data: 0,
            ai: request.ai,
            ai_params: request.ai_params,
            auto_jump: request.auto_jump,
            spawn_rule: String::new(),
            custom_type: String::new(),
            look_at_players: false,
            main_hand_event: request.main_hand_event,
            off_hand_event: request.off_hand_event,
            attack_event: request.attack_event,
        };
        let spawned = match self.entities.spawn_local(spawn) {
            Ok(entity) => entity,
            Err(err) => {
                log::warn!(
                    "plugin entity upsert failed: key={}, error={err:#}",
                    request.key
                );
                return -1;
            }
        };
        if let Err(err) =
            self.entities
                .send_spawn_to_rendered_viewers(&self.players, &self.rendering, &spawned)
        {
            log::warn!(
                "plugin entity spawn packet failed: key={}, error={err:#}",
                request.key
            );
            return -1;
        }
        0
    }

    fn move_entity(&self, plugin_name: &str, query: &str) -> i32 {
        let Some(request) = ParsedEntityMove::parse(plugin_name, query) else {
            return -1;
        };
        let moved = match self
            .entities
            .move_entity_local(&request.key, request.position)
        {
            Ok(entity) => entity,
            Err(err) => {
                log::warn!(
                    "plugin entity move failed: key={}, error={err:#}",
                    request.key
                );
                return 1;
            }
        };
        if moved.dimension != request.dimension {
            log::warn!(
                "plugin entity move dimension mismatch: key={}, expected={}, actual={}",
                request.key,
                request.dimension,
                moved.dimension
            );
            return -1;
        }
        if let Err(err) =
            self.entities
                .send_move_to_rendered_viewers(&self.players, &self.rendering, &moved)
        {
            log::warn!(
                "plugin entity move packet failed: key={}, error={err:#}",
                request.key
            );
            return -1;
        }
        0
    }

    fn remove(&self, plugin_name: &str, query: &str) -> i32 {
        let Some(key) = plugin_entity_key(plugin_name, query.trim()) else {
            return -1;
        };
        let entity = match self.entities.remove_local(&key) {
            Ok(entity) => entity,
            Err(_) => return 1,
        };
        if let Err(err) =
            self.entities
                .send_remove_to_rendered_viewers(&self.players, &self.rendering, &entity)
        {
            log::warn!("plugin entity remove packet failed: key={key}, error={err:#}");
            return -1;
        }
        0
    }
}

#[derive(Debug)]
struct ParsedEntityUpsert {
    key: String,
    name: String,
    dimension: String,
    entity_type: String,
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    display_name: String,
    ai: String,
    ai_params: std::collections::BTreeMap<String, serde_json::Value>,
    auto_jump: bool,
    main_hand_event: String,
    off_hand_event: String,
    attack_event: String,
}

impl ParsedEntityUpsert {
    fn parse(plugin_name: &str, query: &str) -> Option<Self> {
        let mut parts = query.splitn(15, '\t');
        let raw_key = parts.next()?.trim();
        let key = plugin_entity_key(plugin_name, raw_key)?;
        let dimension = parts.next()?.trim().to_string();
        let entity_type = normalize_plugin_resource_key(parts.next()?);
        let x = parts.next()?.trim().parse().ok()?;
        let y = parts.next()?.trim().parse().ok()?;
        let z = parts.next()?.trim().parse().ok()?;
        let yaw = parts.next()?.trim().parse().ok()?;
        let pitch = parts.next()?.trim().parse().ok()?;
        let display_name = parts.next().unwrap_or_default().trim().to_string();
        let ai = parts.next().unwrap_or_default().trim().to_string();
        let ai_params = parse_plugin_ai_params(parts.next().unwrap_or_default())?;
        let auto_jump = parts.next().map(parse_bool_flag).unwrap_or(Some(false))?;
        let main_hand_event = parts.next().unwrap_or_default().trim().to_string();
        let off_hand_event = parts.next().unwrap_or_default().trim().to_string();
        let attack_event = parts.next().unwrap_or_default().trim().to_string();
        if dimension.is_empty() || entity_type.is_empty() {
            return None;
        }
        Some(Self {
            key,
            name: raw_key.to_string(),
            dimension,
            entity_type,
            position: qexed_protocol::to_client::play::add_entity::EntityPosition {
                x,
                y,
                z,
                yaw,
                pitch,
                on_ground: false,
            },
            display_name,
            ai,
            ai_params,
            auto_jump,
            main_hand_event,
            off_hand_event,
            attack_event,
        })
    }
}

fn parse_plugin_ai_params(
    value: &str,
) -> Option<std::collections::BTreeMap<String, serde_json::Value>> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Default::default());
    }
    serde_json::from_str(value).ok()
}

#[derive(Debug)]
struct ParsedEntityMove {
    key: String,
    dimension: String,
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
}

impl ParsedEntityMove {
    fn parse(plugin_name: &str, query: &str) -> Option<Self> {
        let mut parts = query.split('\t');
        let key = plugin_entity_key(plugin_name, parts.next()?.trim())?;
        let dimension = parts.next()?.trim().to_string();
        let x = parts.next()?.trim().parse().ok()?;
        let y = parts.next()?.trim().parse().ok()?;
        let z = parts.next()?.trim().parse().ok()?;
        let yaw = parts.next()?.trim().parse().ok()?;
        let pitch = parts.next()?.trim().parse().ok()?;
        if dimension.is_empty() {
            return None;
        }
        Some(Self {
            key,
            dimension,
            position: qexed_protocol::to_client::play::add_entity::EntityPosition {
                x,
                y,
                z,
                yaw,
                pitch,
                on_ground: false,
            },
        })
    }
}

fn plugin_entity_key(plugin_name: &str, local_key: &str) -> Option<String> {
    let plugin_name = clean_plugin_entity_part(plugin_name)?;
    let local_key = clean_plugin_entity_part(local_key)?;
    Some(format!("plugin:{plugin_name}:{local_key}"))
}

fn clean_plugin_entity_part(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return None;
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/'))
    {
        Some(value.to_string())
    } else {
        None
    }
}

#[derive(Debug)]
struct ParsedWorldEditQuery {
    dimension: String,
    position: qexed_packet::net_types::Position,
    block_state: i32,
}

#[derive(Debug)]
struct ParsedWorldEditRegion;

impl ParsedWorldEditRegion {
    fn parse(query: &str) -> Option<crate::world::RuntimeEditRegion> {
        let mut parts = query.split_whitespace();
        let id = parts.next()?.to_string();
        let dimension = parts.next()?.to_string();
        let min_x = parts.next()?.parse().ok()?;
        let max_x = parts.next()?.parse().ok()?;
        let min_y = parts.next()?.parse().ok()?;
        let max_y = parts.next()?.parse().ok()?;
        let min_z = parts.next()?.parse().ok()?;
        let max_z = parts.next()?.parse().ok()?;
        let allow_player_break = parse_bool_flag(parts.next()?)?;
        let allow_player_place = parse_bool_flag(parts.next()?)?;
        let allow_plugin_write = parse_bool_flag(parts.next()?)?;
        let runtime_only = parse_bool_flag(parts.next()?)?;
        if id.trim().is_empty() || dimension.trim().is_empty() {
            return None;
        }
        Some(crate::world::RuntimeEditRegion {
            id,
            dimension,
            min_x,
            max_x,
            min_y,
            max_y,
            min_z,
            max_z,
            allow_player_break,
            allow_player_place,
            allow_plugin_write,
            runtime_only,
        })
    }
}

fn parse_bool_flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

impl ParsedWorldEditQuery {
    fn parse_set(query: &str) -> Option<Self> {
        let mut parts = query.split_whitespace();
        let dimension = parts.next()?.to_string();
        let position = qexed_packet::net_types::Position {
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
            z: parts.next()?.parse().ok()?,
        };
        let block = parts.next()?;
        let block_state = parse_plugin_block_state(block)?;
        Some(Self {
            dimension,
            position,
            block_state,
        })
    }

    fn parse_break(query: &str) -> Option<Self> {
        let mut parts = query.split_whitespace();
        let dimension = parts.next()?.to_string();
        let position = qexed_packet::net_types::Position {
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
            z: parts.next()?.parse().ok()?,
        };
        Some(Self {
            dimension,
            position,
            block_state: crate::inventory::air_block_state(),
        })
    }
}

fn parse_plugin_block_state(block: &str) -> Option<i32> {
    block.parse::<i32>().ok().or_else(|| {
        let block = normalize_plugin_resource_key(block);
        crate::world::chunk_nbt::default_block_state_id_if_known(&block)
    })
}

fn normalize_plugin_resource_key(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

const PATHFINDING_CACHE_TTL: Duration = Duration::from_millis(1000);
const PATHFINDING_CACHE_LIMIT: usize = 512;

#[derive(Debug, Clone)]
struct ParsedPathQuery {
    dimension: String,
    start: qexed_packet::net_types::Position,
    goal: qexed_packet::net_types::Position,
    max_nodes: usize,
}

impl ParsedPathQuery {
    fn parse(query: &str) -> Option<Self> {
        let mut parts = query.split_whitespace();
        Some(Self {
            dimension: parts.next()?.to_string(),
            start: qexed_packet::net_types::Position {
                x: parts.next()?.parse().ok()?,
                y: parts.next()?.parse().ok()?,
                z: parts.next()?.parse().ok()?,
            },
            goal: qexed_packet::net_types::Position {
                x: parts.next()?.parse().ok()?,
                y: parts.next()?.parse().ok()?,
                z: parts.next()?.parse().ok()?,
            },
            max_nodes: parts
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(4096),
        })
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct PathfindingCacheKey {
    epoch: u64,
    dimension: String,
    start_x: i32,
    start_y: i32,
    start_z: i32,
    goal_x: i32,
    goal_y: i32,
    goal_z: i32,
    max_nodes: usize,
}

impl PathfindingCacheKey {
    fn new(epoch: u64, query: &ParsedPathQuery) -> Self {
        Self {
            epoch,
            dimension: query.dimension.clone(),
            start_x: query.start.x,
            start_y: query.start.y,
            start_z: query.start.z,
            goal_x: query.goal.x,
            goal_y: query.goal.y,
            goal_z: query.goal.z,
            max_nodes: query.max_nodes,
        }
    }
}

#[derive(Debug, Clone)]
struct CachedPath {
    value: Option<String>,
    cached_at: Instant,
}

#[derive(Debug, Default)]
struct PathfindingCache {
    entries: HashMap<PathfindingCacheKey, CachedPath>,
    order: VecDeque<PathfindingCacheKey>,
}

impl PathfindingCache {
    fn get(&mut self, key: &PathfindingCacheKey, now: Instant) -> Option<Option<String>> {
        match self.entries.get(key) {
            Some(entry) if now.duration_since(entry.cached_at) <= PATHFINDING_CACHE_TTL => {
                let value = entry.value.clone();
                self.touch(key.clone());
                Some(value)
            }
            Some(_) => {
                self.remove(key);
                None
            }
            None => None,
        }
    }

    fn insert(&mut self, key: PathfindingCacheKey, value: Option<String>, now: Instant) {
        self.entries.insert(
            key.clone(),
            CachedPath {
                value,
                cached_at: now,
            },
        );
        self.touch(key.clone());
        while self.entries.len() > PATHFINDING_CACHE_LIMIT {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if oldest != key {
                self.entries.remove(&oldest);
            }
        }
    }

    fn remove(&mut self, key: &PathfindingCacheKey) {
        self.entries.remove(key);
        self.order.retain(|existing| existing != key);
    }

    fn touch(&mut self, key: PathfindingCacheKey) {
        self.order.retain(|existing| existing != &key);
        self.order.push_back(key);
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
                    entity_type: npc.entity_type.clone(),
                    entity_type_id_override: None,
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
                    ai: String::new(),
                    ai_params: Default::default(),
                    auto_jump: false,
                    spawn_rule: String::new(),
                    custom_type: String::new(),
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
