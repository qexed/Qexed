//! 服务器组装层：把七个功能域的真实管理器粘成 ServerRuntime + ConnectionHandler。

use std::sync::Arc;

use qexed_server::config::ServerConfig;
use qexed_server::context::{ServerRuntime, ServerServices};
use qexed_server::error::Result as ServerResult;
use qexed_server::server::ConnectionHandler;

use qexed_connection::connection::ServerContext as ConnectionContext;
use qexed_entities::EntityManager;
use qexed_player::PlayerManager;
use qexed_world::world::WorldManager;
use qexed_plugins::PluginManager;

/// 组装后的真实运行时（v4 ServerContext 的 v6 复活）。
pub struct QexedRuntime {
    #[allow(dead_code)]
    pub config: ServerConfig,
    #[allow(dead_code)]
    pub world: Arc<WorldManager>,
    #[allow(dead_code)]
    pub players: Arc<PlayerManager>,
    pub entities: Arc<EntityManager>,
    pub plugins: Arc<PluginManager>,
}

impl std::fmt::Debug for QexedRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QexedRuntime").finish_non_exhaustive()
    }
}

impl QexedRuntime {
    pub fn new(config: ServerConfig, save_path: &str) -> anyhow::Result<Self> {
        use qexed_config::Config as _;
        use qexed_world::world::{WorldLightAlgorithm, WorldLightMode, WorldManager};

        // 世界：按 world.toml 配置选生成器
        let world_config = qexed_world::config::WorldConfig::load_and_create_default(true)?;
        let generator = qexed_world::world::generator::from_config(&world_config);
        let world = Arc::new(WorldManager::with_generator(
            save_path,
            WorldLightMode::default(),
            WorldLightAlgorithm::default(),
            false,
            generator,
        ));

        let players = Arc::new(PlayerManager::new(Arc::new(
            qexed_protocol::types::EntityIdAllocator::default(),
        )));
        let entities = Arc::new(EntityManager::from_config(
            &qexed_entities::config::Entities::default(),
            Arc::new(qexed_entities::EntityIdAllocator::default()),
        )?);
        let plugins = session_plugins().clone();
        Ok(Self { config, world, players, entities, plugins })
    }
}

impl qexed_server::cluster_entities::ClusterPlayers for QexedRuntime {
    fn player_snapshots(&self) -> Vec<qexed_server::cluster_rpc::ClusterPlayerSnapshot> {
        // PlayerManager → ClusterPlayerSnapshot 快照投影（v4 list_except(nil) 的全量版）。
        self.players
            .list_except(uuid::Uuid::nil())
            .into_iter()
            .map(|player| qexed_server::cluster_rpc::ClusterPlayerSnapshot {
                profile_id: *player.profile.uuid.as_bytes(),
                entity_id: player.entity_id,
                username: player.profile.username.clone(),
                dimension: player.dimension.clone(),
                x: player.position.x,
                y: player.position.y,
                z: player.position.z,
                yaw: player.position.yaw,
                pitch: player.position.pitch,
                on_ground: player.position.on_ground,
            })
            .collect()
    }

    fn send_packets_to(&self, profile_id: uuid::Uuid, packets: Vec<bytes::Bytes>) {
        // PlayerManager 的 ClientboundPackets 事件通道回到会话 sink。
        self.players.send_packets_to(profile_id, packets);
    }
}

/// WorldManager → 实体域 WorldAccess（方块查询/放置 + 缓存纪元）。
struct RuntimeWorldAccess(std::sync::Arc<WorldManager>);

impl qexed_entities::WorldAccess for RuntimeWorldAccess {
    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        self.0.block_state_at(dimension, position)
    }

    fn cache_epoch(&self) -> u64 {
        self.0.cache_epoch()
    }

    fn place_blocks(
        &self,
        dimension: &str,
        blocks: Vec<(qexed_packet::net_types::Position, i32)>,
    ) -> std::result::Result<
        Vec<qexed_entities::BlockUpdate>,
        qexed_entities::error::EntitiesError,
    > {
        let updates = self.0.place_blocks(dimension, blocks).map_err(|err| {
            // WorldError 无实体域对应变体：经 io::Error::other 包装（消息保留原文）。
            qexed_entities::error::EntitiesError::Io(std::io::Error::other(err.to_string()))
        })?;
        Ok(updates
            .into_iter()
            .map(|update| qexed_entities::BlockUpdate {
                position: update.location,
                block_state: update.block_state.0,
            })
            .collect())
    }
}

/// PluginManager → 实体域 EntityAiHost（payload 字段级转换；
/// entities 与 plugins 各有一套同构 EntityAiTickQuery）。
struct RuntimeEntityAiHost(std::sync::Arc<PluginManager>);

impl qexed_entities::EntityAiHost for RuntimeEntityAiHost {
    fn handle_entity_ai_tick(
        &self,
        query: qexed_entities::EntityAiTickQuery,
    ) -> Vec<qexed_entities::EntityAiOperation> {
        use qexed_plugins::api as plugin_api;
        let dimension = query.entity.dimension.clone();
        let nearby_players = query
            .nearby_players
            .into_iter()
            .map(|player| plugin_api::EntityAiPlayerPayload {
                // AI 查询只消费位置 + entity_id（entities 侧载荷即无 uuid/名字）。
                player: plugin_api::PlayerPayloadOwned {
                    uuid: String::new(),
                    username: String::new(),
                    entity_id: player.entity_id,
                    language: String::new(),
                    dimension: dimension.clone(),
                },
                position: position_payload(player.position),
            })
            .collect();
        let converted = plugin_api::EntityAiTickQuery {
            entity: plugin_api::EntityAiEntityPayload {
                key: query.entity.key,
                entity_id: query.entity.entity_id,
                entity_type: query.entity.entity_type,
                custom_type: query.entity.custom_type,
                ai: query.entity.ai,
                spawn_rule: query.entity.spawn_rule,
                ai_params: query.entity.ai_params,
                dimension: query.entity.dimension,
                position: position_payload(query.entity.position),
            },
            nearby_players,
            tick_ms: query.tick_ms,
        };
        self.0
            .handle_entity_ai_tick(converted)
            .into_iter()
            .map(|operation| match operation {
                plugin_api::EntityAiOperation::MoveDelta { x, y, z, yaw, pitch } => {
                    qexed_entities::EntityAiOperation::MoveDelta { x, y, z, yaw, pitch }
                }
                plugin_api::EntityAiOperation::LookAt { x, y, z } => {
                    qexed_entities::EntityAiOperation::LookAt { x, y, z }
                }
                plugin_api::EntityAiOperation::Remove => {
                    qexed_entities::EntityAiOperation::Remove
                }
            })
            .collect()
    }
}

/// 实体域位置 → 插件事件位置 payload。
fn position_payload(
    position: qexed_entities::EntityPosition,
) -> qexed_plugins::api::PlayerPositionPayload {
    qexed_plugins::api::PlayerPositionPayload {
        x: position.x,
        y: position.y,
        z: position.z,
        yaw: position.yaw,
        pitch: position.pitch,
        on_ground: position.on_ground,
    }
}

/// 全局服务 tick 间隔（v4 services.rs GLOBAL_SERVICE_TICK_INTERVAL 同值）。
const GLOBAL_SERVICE_TICK_MS: u64 = 50;

impl ServerRuntime for QexedRuntime {
    fn server_config(&self) -> &ServerConfig {
        &self.config
    }

    fn global_service_tick(&self) -> ServerResult<()> {
        // v4 本地模式 tick：实体 AI/物理/掉落物（tick_ai 5 参接线）。
        let viewers = qexed_play::plugin_bridge::PlayerViewerBridge::new(&self.players);
        let world = RuntimeWorldAccess(self.world.clone());
        let plugins = RuntimeEntityAiHost(self.plugins.clone());
        self.entities
            .tick_ai(
                &viewers,
                &world,
                &plugins,
                &qexed_entities::EntityRendering::default(),
                GLOBAL_SERVICE_TICK_MS,
            )
            .map_err(|err| qexed_server::error::ServerError::msg(err.to_string()))?;
        Ok(())
    }

    fn plugin_summaries(&self) -> Vec<String> {
        self.plugins.plugin_summaries()
    }

    fn emit_config_reload(&self, config_path: &str) {
        self.plugins.emit_config_reload(config_path);
    }
}

/// v4 connection::handle 的 v6 适配。
pub struct QexedConnectionHandler {
    pub context: ConnectionContext,
}

impl ConnectionHandler for QexedConnectionHandler {
    fn handle(
        &self,
        stream: tokio::net::TcpStream,
        addr: std::net::SocketAddr,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let context = self.context.clone();
        Box::pin(async move {
            qexed_connection::connection::handle(stream, addr, context, shutdown).await;
        })
    }
}

/// 全局统计注册表（按玩家 uuid 共享——跨会话累积）。
static STATS_REGISTRY: std::sync::OnceLock<qexed_statistics::counter::StatsRegistry> =
    std::sync::OnceLock::new();

fn stats_registry() -> &'static qexed_statistics::counter::StatsRegistry {
    STATS_REGISTRY.get_or_init(qexed_statistics::counter::StatsRegistry::new)
}

/// 全局插件管理器（QexedRuntime 与 play 会话共享同一实例，避免重复加载）。
static SESSION_PLUGINS: std::sync::OnceLock<Arc<PluginManager>> = std::sync::OnceLock::new();

fn session_plugins() -> &'static Arc<PluginManager> {
    SESSION_PLUGINS.get_or_init(|| Arc::new(PluginManager::from_dir("./plugins")))
}

/// 构造 play 会话启动器（注入 connection 的 ServerContext）。
pub fn play_launcher(
    world: std::sync::Arc<qexed_world::world::WorldManager>,
    players: std::sync::Arc<qexed_player::PlayerManager>,
) -> qexed_connection::connection::PlayLauncherFn {
    std::sync::Arc::new(
        move |mut packets: qexed_connection::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>, mut sink: qexed_connection::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>, profile: qexed_packet::net_types::GameProfile, locale: Option<String>, skin_parts: u8, shutdown: tokio::sync::watch::Receiver<bool>| {
            let world = world.clone();
            let players = players.clone();
            Box::pin(async move {
                play_session(&mut packets, &mut sink, &world, &players, profile, locale, skin_parts, shutdown).await
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = qexed_connection::error::Result<()>> + Send>>
        },
    )
}

/// play 会话（v4 crate::play::initialize 的组装层实现）。
#[allow(clippy::too_many_arguments)]
async fn play_session(
    packets: &mut qexed_connection::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    sink: &mut qexed_connection::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
    world: &std::sync::Arc<qexed_world::world::WorldManager>,
    players: &std::sync::Arc<qexed_player::PlayerManager>,
    profile: qexed_packet::net_types::GameProfile,
    locale: Option<String>,
    skin_parts: u8,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> qexed_connection::error::Result<()> {
    use qexed_play::{
        AllowAllPermissions, NoChatFilter, NoGameplay, NoPluginEvents, NoSecureChat,
        PlaySessionDeps,
    };

    let config = qexed_play::PlayConfig::default();
    let player_data = qexed_player::player_data::PlayerDataManager::from_config(
        "./world",
        &qexed_player::config::PlayerDataConfig::default(),
    )
    .await
    .map_err(|e| qexed_connection::error::ConnectionError::msg(e.to_string()))?;
    let entities = std::sync::Arc::new(
        qexed_entities::EntityManager::from_config(
            &qexed_entities::config::Entities::default(),
            std::sync::Arc::new(qexed_entities::EntityIdAllocator::default()),
        )
        .map_err(|e| qexed_connection::error::ConnectionError::msg(e.to_string()))?,
    );
    let inventory = crate::items_registry::RealInventory::new(Default::default());
    let items = crate::items_registry::RealItems::load();
    let world_rules_adapter = crate::world_rules_adapter::RealWorldRules::new(world);
    let gameplay_hooks = crate::gameplay_hooks::RealGameplay::new(
        config.clone(),
        players.clone(),
        entities.clone(),
        qexed_entities::EntityRendering::default(),
        qexed_play::survival::StoredSurvival::default(),
        &qexed_player::StoredInventory::default(),
        world.clone(),
        session_plugins().clone(),
        world_rules_adapter.handle(),
    )
    .with_stats(stats_registry().counter(profile.uuid));
    let display = qexed_play::ServerDisplay::default();
    let shared_world: qexed_play::SharedWorld = crate::world_adapter::shared_world(world.clone());
    // chat 命令依赖：方块结构放置面用真实 WorldManager 适配，权限先全放行
    // （待 server 域 PermissionManager 落地后替换）。
    let structure_sink = crate::world_adapter::RealWorld(world.clone());
    let world_rules_handle = world_rules_adapter.handle();
    let all_commands = AllCommandsAllowed;

    let deps = PlaySessionDeps {
        config: &config,
        world: shared_world,
        world_rules: &world_rules_adapter,
        players,
        player_data: &player_data,
        entities: &entities,
        plugins: &NoPluginEvents,
        permissions: &AllowAllPermissions,
        player_audit: &qexed_play::NoPlayerAudit,
        items: &items,
        inventory: &inventory,
        gameplay: &gameplay_hooks,
        secure_chat: &NoSecureChat,
        chat_filter: &NoChatFilter,
        commands: Some(qexed_play::ChatCommandDeps {
            world_structure: &structure_sink,
            world_rules: &world_rules_handle,
            plugins: session_plugins(),
            permissions: &all_commands,
            inventory: gameplay_hooks.inventory_handle(),
        }),
        cluster: None,
        display,
        command_tree: bytes::Bytes::new(),
        entity_rendering: qexed_entities::EntityRendering::default(),
        player_entity_type: 1,
    };

    // 登录推送全量统计（原版统计页初始数据）。
    {
        let mut entries: Vec<(qexed_statistics::StatKey, i64)> = gameplay_hooks
            .stats
            .snapshot()
            .into_iter()
            .filter_map(|(name, value)| qexed_statistics::persist::parse_full_name(&name).map(|key| (key, value)))
            .collect();
        {
            // 原版统计页数据源：无条件推送（空统计也让客户端页面可用）。
            if entries.is_empty() {
                entries.push((
                    qexed_statistics::StatKey::custom(qexed_statistics::CUSTOM_PLAY_TIME),
                    gameplay_hooks.stats.get(&qexed_statistics::StatKey::custom(
                        qexed_statistics::CUSTOM_PLAY_TIME,
                    )),
                ));
            }
            match qexed_statistics::packet::award_stats_packet(&entries) {
                Ok(stats_packet) => {
                    use qexed_packet::{Packet, PacketCodec};
                    let mut buf = bytes::BytesMut::new();
                    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
                    let encode = qexed_packet::net_types::VarInt(
                        <qexed_protocol::to_client::play::award_stats::AwardStats as qexed_packet::Packet>::ID,
                    )
                    .serialize(&mut writer)
                    .and_then(|()| stats_packet.serialize(&mut writer));
                    if encode.is_ok() {
                        let _ = sink.send_raw(buf.freeze()).await;
                    }
                }
                Err(err) => log::debug!("stats push encode failed: {err}"),
            }
        }
    }

    qexed_play::initialize(
        packets,
        sink,
        &deps,
        &profile,
        locale,
        skin_parts,
        shutdown,
    )
    .await
    .map_err(|e| qexed_connection::error::ConnectionError::msg(e.to_string()))
}

/// 命令权限默认全放行（v4 默认权限表等价；待 server 域 PermissionManager 落地后替换）。
struct AllCommandsAllowed;

impl qexed_play::chat_support::CommandPermissions for AllCommandsAllowed {
    fn can_run_command(
        &self,
        _profile: &qexed_packet::net_types::GameProfile,
        _command: &str,
    ) -> bool {
        true
    }
}
