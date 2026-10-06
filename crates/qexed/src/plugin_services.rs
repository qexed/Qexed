//! 插件宿主服务实现（v4 connection/context.rs 后半部分的服务实现的 v6 归属地）。
//!
//! v4 里 ServerPathfindingService / ServerWorldEditService / ServerEntityControlService
//! 直接写在 connection::context（单 crate 无依赖方向问题）。v6 拆分后这些实现
//! 依赖 qexed_world / qexed_entities / qexed_player / qexed_play 的具体类型，
//! 不能放进 qexed_connection（依赖方向：connection 不得依赖这些域），因此落在
//! qexed 组装层，经 PluginManager::set_*_service 注入 qexed_plugins 的宿主槽位。
//!
//! v4 的 apply_plugin_npc_mutations（启动期 NPC 落场）与自定义实体注册也在此
//! 编排，由 runtime::QexedRuntime::ensure_plugins_initialized 在首个玩家配置
//! 完成时触发（经 qexed_connection 的 PluginsInitFn 回调）。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use qexed_packet::net_types::Position;
use qexed_world::world::WorldManager;

use qexed_player::PlayerManager;

use crate::runtime::QexedRuntime;

// ---------------------------------------------------------------------------
// 寻路（v4 ServerPathfindingService + PathfindingCache）
// ---------------------------------------------------------------------------

const PATHFINDING_CACHE_TTL: Duration = Duration::from_millis(1000);
const PATHFINDING_CACHE_LIMIT: usize = 512;

/// 寻路服务：qexed_play::pathfinding 经 WorldBlockSource 适配 WorldManager，
/// 外面套 v4 同款 TTL + LRU 缓存。
#[derive(Debug)]
struct ServerPathfindingService {
    world: Arc<WorldManager>,
    cache: Mutex<PathfindingCache>,
}

impl qexed_plugins::host::PathfindingService for ServerPathfindingService {
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

        let path = qexed_play::pathfinding::find_path(qexed_play::pathfinding::PathQuery {
            world: &crate::world_adapter::RealWorld(self.world.clone()),
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

/// v4 ParsedPathQuery：空格分隔 dimension sx sy sz gx gy gz [max_nodes]。
#[derive(Debug, Clone)]
struct ParsedPathQuery {
    dimension: String,
    start: Position,
    goal: Position,
    max_nodes: usize,
}

impl ParsedPathQuery {
    fn parse(query: &str) -> Option<Self> {
        let mut parts = query.split_whitespace();
        Some(Self {
            dimension: parts.next()?.to_string(),
            start: Position {
                x: parts.next()?.parse().ok()?,
                y: parts.next()?.parse().ok()?,
                z: parts.next()?.parse().ok()?,
            },
            goal: Position {
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

/// v4 PathfindingCache：TTL 失效 + 访问序 LRU 淘汰。
#[derive(Debug, Default)]
struct PathfindingCache {
    entries: std::collections::HashMap<PathfindingCacheKey, CachedPath>,
    order: std::collections::VecDeque<PathfindingCacheKey>,
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

// ---------------------------------------------------------------------------
// World Edit（v4 ServerWorldEditService）
// ---------------------------------------------------------------------------

/// 世界编辑服务：解析 v4 同款文本协议，写入 WorldManager 并向在线玩家广播。
/// 玩家广播面用 PlayerManager::broadcast_block_changed（v4 players 同名能力）。
#[derive(Debug)]
struct ServerWorldEditService {
    world: Arc<WorldManager>,
    players: Arc<PlayerManager>,
}

impl qexed_plugins::host::WorldEditService for ServerWorldEditService {
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
            request.position,
            request.block_state,
            region.runtime_only,
        );
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
            self.write_block(&request.dimension, request.position, request.block_state, runtime_only);
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
        self.write_block(&request.dimension, request.position, AIR_BLOCK_STATE, region.runtime_only);
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
        position: Position,
        block_state: i32,
        runtime_only: bool,
    ) {
        if runtime_only || self.world.read_only() {
            self.world
                .set_runtime_block(dimension, position.clone(), block_state);
        } else {
            self.world.place_block(dimension, position.clone(), block_state);
        }
        // 广播给在线玩家（v4 broadcast_change；光照包由 PlayerManager 侧按需补发）。
        self.players.broadcast_block_changed(
            uuid::Uuid::nil(),
            dimension,
            position,
            block_state,
            None,
        );
    }
}

/// 空气方块状态 id（原版全量表为 0；与 qexed_play::world_access::default_block_state_id("minecraft:air") 一致）。
const AIR_BLOCK_STATE: i32 = 0;

/// v4 ParsedWorldEditQuery：set 为空格分隔 dimension x y z block；break 无 block。
#[derive(Debug)]
struct ParsedWorldEditQuery {
    dimension: String,
    position: Position,
    block_state: i32,
}

impl ParsedWorldEditQuery {
    fn parse_set(query: &str) -> Option<Self> {
        let mut parts = query.split_whitespace();
        let dimension = parts.next()?.to_string();
        let position = Position {
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
        let position = Position {
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
            z: parts.next()?.parse().ok()?,
        };
        Some(Self {
            dimension,
            position,
            block_state: AIR_BLOCK_STATE,
        })
    }
}

/// v4 ParsedWorldEditRegion：id dimension min_x max_x min_y max_y min_z max_z
/// allow_break allow_place allow_plugin runtime_only。
struct ParsedWorldEditRegion;

impl ParsedWorldEditRegion {
    fn parse(query: &str) -> Option<qexed_world::world::RuntimeEditRegion> {
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
        Some(qexed_world::world::RuntimeEditRegion {
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

/// v4 parse_plugin_block_state：纯数字直接用；否则 名字[属性=值,...] 查
/// qexed_play::world_access 的方块状态表（同源 blocks.json）。
fn parse_plugin_block_state(block: &str) -> Option<i32> {
    if let Ok(id) = block.parse::<i32>() {
        return Some(id);
    }
    let (name, properties) = parse_plugin_block_state_name_and_properties(block)?;
    if properties.is_empty() {
        return Some(qexed_play::world_access::default_block_state_id(&name));
    }
    let state = qexed_play::world_access::block_state(&name, &properties);
    let entry = qexed_play::world_access::block_state_entry(state);
    (entry.name == name && entry.properties == properties).then_some(state)
}

fn parse_plugin_block_state_name_and_properties(
    block: &str,
) -> Option<(String, Vec<(String, String)>)> {
    let block = block.trim();
    let Some((name, raw_properties)) = block.split_once('[') else {
        return Some((normalize_plugin_resource_key(block), Vec::new()));
    };
    let raw_properties = raw_properties.strip_suffix(']')?;
    let mut properties = Vec::new();
    for raw_property in raw_properties.split(',') {
        let (key, value) = raw_property.split_once('=')?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return None;
        }
        properties.push((key.to_string(), value.to_string()));
    }
    properties.sort_by(|left, right| left.0.cmp(&right.0));
    Some((normalize_plugin_resource_key(name), properties))
}

fn normalize_plugin_resource_key(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

// ---------------------------------------------------------------------------
// 初始化编排（v4 ServerContext::new 的插件服务装配 + ensure_plugins_initialized）
// ---------------------------------------------------------------------------

impl QexedRuntime {
    /// 把宿主服务实现注入 PluginManager（v4 ServerContext::new 内联装配）。
    /// 幂等：PluginManager 侧 set_*_service 直接覆盖槽位。
    pub(crate) fn install_plugin_services(&self) {
        self.plugins.set_pathfinding_service(Arc::new(
            ServerPathfindingService {
                world: self.world.clone(),
                cache: Mutex::new(PathfindingCache::default()),
            },
        ));
        self.plugins.set_world_edit_service(Arc::new(ServerWorldEditService {
            world: self.world.clone(),
            players: self.players.clone(),
        }));
        // v6 尚未迁移 v4 的实体控制宿主（EntityManager 的插件视图需要渲染/广播面，
        // TODO(entities)：EntityManager 提供 send_*_to_rendered_viewers 后补
        // ServerEntityControlService 与 apply_plugin_npc_mutations）。
    }

    /// v4 ensure_plugins_initialized：ensure_initialized（load+init+config+language 事件）
    /// + 注册插件自定义实体 + 应用启动期 NPC 变更。只跑一次。
    pub(crate) fn ensure_plugins_initialized(&self, language: &str) {
        self.plugins.ensure_initialized(language);
        let definitions = self.plugins.custom_entities().into_iter().map(|definition| {
            qexed_entities::CustomEntityDefinition {
                id: definition.id,
                entity_type: definition.entity_type,
                shell_entity_type: definition.shell_entity_type,
                registry_id: definition.registry_id,
                display_name: definition.display_name,
                ai: definition.ai,
                ai_params: definition.ai_params,
            }
        });
        self.entities.register_custom_entities(definitions);
        // v4 apply_plugin_npc_mutations("startup")：依赖实体广播面，随实体控制服务一起补。
    }

    /// 构造注入 qexed_connection 的插件初始化回调。
    pub fn plugins_init_callback(self: &Arc<Self>, language: &str) ->
        qexed_connection::connection::PluginsInitFn
    {
        let runtime = self.clone();
        let language = language.to_string();
        Arc::new(move || runtime.ensure_plugins_initialized(&language))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_block_state_parser_supports_properties() {
        let parsed = parse_plugin_block_state_name_and_properties("ladder[facing=south]").unwrap();
        assert_eq!(parsed.0, "minecraft:ladder");
        assert_eq!(parsed.1, vec![("facing".to_string(), "south".to_string())]);

        let parsed = parse_plugin_block_state_name_and_properties(
            "minecraft:oak_trapdoor[waterlogged=false,facing=north]",
        )
        .unwrap();
        assert_eq!(parsed.0, "minecraft:oak_trapdoor");
        assert_eq!(
            parsed.1,
            vec![
                ("facing".to_string(), "north".to_string()),
                ("waterlogged".to_string(), "false".to_string()),
            ]
        );
    }

    #[test]
    fn plugin_block_state_parser_rejects_malformed_properties() {
        assert!(parse_plugin_block_state_name_and_properties("ladder[facing]").is_none());
        assert!(parse_plugin_block_state_name_and_properties("ladder[facing=south").is_none());
    }

    #[test]
    fn pathfinding_query_parses_optional_max_nodes() {
        let query = ParsedPathQuery::parse("minecraft:overworld 0 64 0 10 64 10").unwrap();
        assert_eq!(query.max_nodes, 4096);

        let query = ParsedPathQuery::parse("minecraft:overworld 0 64 0 10 64 10 64").unwrap();
        assert_eq!(query.max_nodes, 64);
    }

    #[test]
    fn bool_flag_accepts_common_spellings() {
        assert_eq!(parse_bool_flag("1"), Some(true));
        assert_eq!(parse_bool_flag("false"), Some(false));
        assert_eq!(parse_bool_flag("ON"), Some(true));
        assert_eq!(parse_bool_flag("junk"), None);
    }

    #[test]
    fn pathfinding_cache_ttl_and_lru() {
        let mut cache = PathfindingCache::default();
        let key = PathfindingCacheKey::new(0, &ParsedPathQuery::parse("d 0 0 0 1 0 1").unwrap());
        cache.insert(key.clone(), Some("0,0,0;1,0,1".to_string()), Instant::now());

        assert_eq!(
            cache.get(&key, Instant::now()),
            Some(Some("0,0,0;1,0,1".to_string()))
        );
        // TTL 过期后命中失效。
        let later = Instant::now() + PATHFINDING_CACHE_TTL + Duration::from_millis(1);
        assert_eq!(cache.get(&key, later), None);
    }
}
