//! 连接域上下文（v4 connection::context 迁移）。
//!
//! v4 的 ServerContext 是单体（world/players/entities/plugins/warden/resource_pack
//! /audit/content_filter/profiler 等全部塞在一起）。v6 按域拆分 crate 后这些
//! 管理器属于 qexed_world/qexed_player/qexed_entities/qexed_plugins/qexed_server，
//! 连接域只保留自己需要的状态；跨域能力以 trait 接口留出（TODO(hook)），
//! 由 qexed_server 在组装层实现注入，避免依赖环。

use std::sync::Arc;

use crate::{
    auth::Authenticator,
    code_of_conduct::{DEFAULT_CODE_OF_CONDUCT_DIR, CodeOfConductTexts},
    config::ConnectionConfig,
};

/// 连接域上下文：配置 + 认证器 + 行为守则文本。
#[derive(Clone)]
pub struct ServerContext {
    pub config: Arc<ConnectionConfig>,
    pub authenticator: Arc<Authenticator>,
    pub code_of_conducts: Arc<CodeOfConductTexts>,
    /// play 会话启动器（qexed 组装层注入；None 则登录后断开）。
    pub play_launcher: Option<PlayLauncherFn>,
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

impl ServerContext {
    /// 组装层注入 play 会话启动器。
    pub fn set_play_launcher(&mut self, launcher: PlayLauncherFn) {
        self.play_launcher = Some(launcher);
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
            config: Arc::new(config),
            authenticator: Arc::new(Authenticator::new()?),
            code_of_conducts,
        })
    }

    /// v4 在登录完成后调用 ensure_plugins_initialized 触发插件启动应用；
    /// v6 插件域在 qexed_plugins，此处留空 hook。
    /// TODO(hook): qexed_server 组装时改为调用插件域的初始化入口。
    pub fn ensure_plugins_initialized(&self) {}
}

// v4 context.rs 其余内容（插件宿主服务）不在此迁移：
// TODO(hook): v4 的 ServerPathfindingService / ServerWorldEditService /
// ServerEntityControlService / ParsedEntityUpsert / ParsedWorldEditQuery /
// PathfindingCache / apply_plugin_npc_mutations 等插件宿主服务依赖
// qexed_world / qexed_entities / qexed_play / qexed_plugins 的实际实现，
// 属于插件域能力。它们应在 qexed_plugins（服务定义）与 qexed_server（宿主实现）
// 中迁移；连接域不再承载。对应 v4 文件：src/connection/context.rs 后半部分。
