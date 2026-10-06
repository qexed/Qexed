//! 连接域上下文（v4 connection::context 迁移）。
//!
//! v4 的 ServerContext 是单体（world/players/entities/plugins/warden/resource_pack
//! /audit/content_filter/profiler 等全部塞在一起）。v6 按域拆分 crate 后这些
//! 管理器属于 qexed_world/qexed_player/qexed_entities/qexed_plugins/qexed_server，
//! 连接域只保留自己需要的状态；跨域能力（play 启动 / 封禁查询 / 插件初始化 /
//! 资源包解析）以回调注入，由 qexed 组装层接线，避免依赖环。

use std::sync::Arc;

use crate::{
    auth::Authenticator,
    code_of_conduct::{DEFAULT_CODE_OF_CONDUCT_DIR, CodeOfConductTexts},
    config::ConnectionConfig,
};

/// 连接域上下文：配置 + 认证器 + 行为守则文本 + 可选跨域回调。
#[derive(Clone)]
pub struct ServerContext {
    pub config: Arc<ConnectionConfig>,
    pub authenticator: Arc<Authenticator>,
    pub code_of_conducts: Arc<CodeOfConductTexts>,
    /// play 会话启动器（qexed 组装层注入；None 则登录后断开）。
    pub play_launcher: Option<PlayLauncherFn>,
    pub ban_check: Option<BanCheckFn>,
    /// 资源包推送解析回调（组装层注入；None 用连接域默认 resolve_offer）。
    pub resource_pack_offer: Option<ResourcePackOfferFn>,
    /// 插件域初始化回调（qexed_plugins 经组装层注入；None 跳过）。
    plugins_init: Option<PluginsInitFn>,
    /// 插件初始化只跑一次（v4 plugin_startup_applied OnceLock 的等价物）。
    plugin_startup_applied: Arc<std::sync::OnceLock<()>>,
}

/// play 会话启动器签名（v4 crate::play::initialize 的注入版）。
/// 传输以 trait object 注入，避免具体流类型耦合。
pub type PlayLauncherFn = std::sync::Arc<
    dyn Fn(
            crate::transport::PacketStream<tokio::io::ReadHalf<tokio::net::TcpStream>>,
            crate::transport::PacketSink<tokio::io::WriteHalf<tokio::net::TcpStream>>,
            qexed_packet::net_types::GameProfile,
            Option<String>,
            u8,
            tokio::sync::watch::Receiver<bool>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<()>> + Send>>
        + Send
        + Sync,
>;

/// 封禁检查回调：返回 Some(原因) 表示拒绝登录。
pub type BanCheckFn = std::sync::Arc<dyn Fn(uuid::Uuid) -> Option<String> + Send + Sync>;

/// 资源包推送解析回调：按配置与登录主机算出 (url, hash)；None 表示跳过推送。
/// 组装层注入的异步版本（对象存储上传等需要 IO 的实现用这个）。
pub type ResourcePackOfferFn = std::sync::Arc<
    dyn Fn(
            &crate::config::ResourcePack,
            &str,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = std::result::Result<Option<(String, String)>, String>> + Send>,
        > + Send
        + Sync,
>;

/// 插件域初始化回调（qexed_plugins::PluginManager::ensure_initialized 的注入版）。
/// 语言等参数由组装层闭包捕获，连接域不感知插件域配置。
pub type PluginsInitFn = std::sync::Arc<dyn Fn() + Send + Sync>;

impl ServerContext {
    /// 组装层注入 play 会话启动器。
    pub fn set_ban_check(&mut self, check: BanCheckFn) {
        self.ban_check = Some(check);
    }

    pub fn set_play_launcher(&mut self, launcher: PlayLauncherFn) {
        self.play_launcher = Some(launcher);
    }

    /// 组装层注入资源包推送解析回调（覆盖连接域默认 resolve_offer）。
    pub fn set_resource_pack_offer(&mut self, resolve: ResourcePackOfferFn) {
        self.resource_pack_offer = Some(resolve);
    }

    /// 组装层注入插件域初始化回调（qexed_plugins 的 ensure_initialized）。
    pub fn set_plugins_init(&mut self, init: PluginsInitFn) {
        self.plugins_init = Some(init);
    }

    /// 从配置构造（行为守则目录用默认值）。
    pub async fn new(config: impl Into<ConnectionConfig>) -> crate::error::Result<Self> {
        Self::new_with_code_of_conduct_dir(config, DEFAULT_CODE_OF_CONDUCT_DIR).await
    }

    /// 从配置构造，指定行为守则目录（v4 同名方法迁移）。
    /// v4 这里还会构建 world/players/entities 等全部运行时，v6 移到 qexed_server。
    pub async fn new_with_code_of_conduct_dir(
        config: impl Into<ConnectionConfig>,
        code_of_conduct_dir: impl AsRef<std::path::Path>,
    ) -> crate::error::Result<Self> {
        let config = config.into();
        let code_of_conducts = Arc::new(CodeOfConductTexts::load(
            config.code_of_conduct,
            code_of_conduct_dir,
        )?);
        Ok(Self {
            play_launcher: None,
            ban_check: None,
            resource_pack_offer: None,
            plugins_init: None,
            plugin_startup_applied: Arc::new(std::sync::OnceLock::new()),
            config: Arc::new(config),
            authenticator: Arc::new(Authenticator::new()?),
            code_of_conducts,
        })
    }

    /// v4 在登录完成后调用 ensure_plugins_initialized 触发插件启动应用。
    /// v6 插件域在 qexed_plugins：由组装层经 set_plugins_init 注入其
    /// ensure_initialized（语言参数由组装层闭包捕获 ServerConfig.language）；
    /// 未注入时无操作。OnceLock 保证多连接并发下只初始化一次。
    pub fn ensure_plugins_initialized(&self) {
        if let Some(init) = &self.plugins_init {
            self.plugin_startup_applied.get_or_init(|| init());
        }
    }
}

// v4 context.rs 后半部分（插件宿主服务）不迁移到连接域，归属如下：
// - 服务 trait 定义（PathfindingService / WorldEditService /
//   EntityControlService / LocalizeService / PluginApiService）已在
//   qexed_plugins::host 定义（v6 为 libloading 动态库插件宿主）。
// - 宿主实现（v4 的 ServerPathfindingService / ServerWorldEditService /
//   ServerEntityControlService / PathfindingCache / apply_plugin_npc_mutations）
//   依赖 qexed_world / qexed_entities / qexed_play 的具体类型，由 qexed 组装层
//   （crates/qexed/src/plugin_services.rs）实现并经
//   PluginManager::set_pathfinding_service / set_world_edit_service /
//   set_entity_control_service 注入；连接域经 set_plugins_init 在首个玩家
//   配置完成时触发整条初始化链。连接域不承载插件宿主能力。
