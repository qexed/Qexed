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
        let plugins = Arc::new(PluginManager::from_dir("./plugins"));
        Ok(Self { config, world, players, entities, plugins })
    }
}

impl qexed_server::cluster_entities::ClusterPlayers for QexedRuntime {
    fn player_snapshots(&self) -> Vec<qexed_server::cluster_rpc::ClusterPlayerSnapshot> {
        // TODO(assembly): PlayerManager → ClusterPlayerSnapshot 快照投影
        Vec::new()
    }

    fn send_packets_to(&self, _profile_id: uuid::Uuid, _packets: Vec<bytes::Bytes>) {
        // TODO(assembly): PlayerManager 原始包发送通道接线
    }
}

impl ServerRuntime for QexedRuntime {
    fn server_config(&self) -> &ServerConfig {
        &self.config
    }

    fn global_service_tick(&self) -> ServerResult<()> {
        // TODO(assembly): tick_ai 需 5 参（世界/规则等），接线后启用
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
    let entities = qexed_entities::EntityManager::from_config(
        &qexed_entities::config::Entities::default(),
        std::sync::Arc::new(qexed_entities::EntityIdAllocator::default()),
    )
    .map_err(|e| qexed_connection::error::ConnectionError::msg(e.to_string()))?;
    let inventory = BasicInventory;
    let display = qexed_play::ServerDisplay::default();
    let shared_world: qexed_play::SharedWorld = crate::world_adapter::shared_world(world.clone());

    let deps = PlaySessionDeps {
        config: &config,
        world: shared_world,
        world_rules: &NoWorldRules,
        players,
        player_data: &player_data,
        entities: &entities,
        plugins: &NoPluginEvents,
        permissions: &AllowAllPermissions,
        player_audit: &qexed_play::NoPlayerAudit,
        items: &BasicItems,
        inventory: &inventory,
        gameplay: &NoGameplay,
        secure_chat: &NoSecureChat,
        chat_filter: &NoChatFilter,
        cluster: None,
        display,
        command_tree: bytes::Bytes::new(),
        entity_rendering: qexed_entities::EntityRendering::default(),
        player_entity_type: 1,
    };

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


/// 最小会话背包（空装备）。
struct BasicInventory;

impl qexed_play::SessionInventory for BasicInventory {
    fn selected_slot(&self) -> usize {
        0
    }

    fn set_player_inventory_packets(&self) -> Vec<bytes::Bytes> {
        Vec::new()
    }

    fn visible_equipment(&self) -> Vec<qexed_protocol::to_client::play::set_equipment::EquipmentEntry> {
        Vec::new()
    }

    fn equipment_packet(
        &self,
        _entity_id: i32,
        _equipment: Vec<qexed_protocol::to_client::play::set_equipment::EquipmentEntry>,
    ) -> qexed_play::Result<Option<qexed_protocol::to_client::play::set_equipment::SetEquipment>> {
        Ok(None)
    }
}

struct BasicItems;

impl qexed_play::ItemRegistry for BasicItems {
    fn is_air_block_state(&self, block_state: i32) -> bool {
        block_state == 0
    }

    fn air_block_state(&self) -> i32 {
        0
    }

    fn picked_item_for_block_state(&self, _block_state: i32) -> Option<i32> {
        None
    }

    fn item_id_for_name(&self, _name: &str) -> Option<i32> {
        None
    }

    fn simple_item(&self, _item_id: i32, _count: i32) -> qexed_protocol::types::Slot {
        qexed_play::inventory::empty_slot()
    }

    fn empty_slot(&self) -> qexed_protocol::types::Slot {
        qexed_play::inventory::empty_slot()
    }
}

struct NoWorldRules;

impl qexed_play::WorldRulesSource for NoWorldRules {
    fn ensure_loaded(&self, _dimension: &str) -> qexed_play::Result<()> {
        Ok(())
    }

    fn snapshot(&self, dimension: &str) -> qexed_play::DimensionRules {
        qexed_play::DimensionRules {
            dimension_type: dimension.to_string(),
        }
    }

    fn current_time(&self, _dimension: &str) -> i64 {
        0
    }

    fn tick_dimension_time(&self, _dimension: &str, default_day_ticks: i64) -> i64 {
        default_day_ticks
    }
}
