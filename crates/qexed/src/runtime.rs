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
