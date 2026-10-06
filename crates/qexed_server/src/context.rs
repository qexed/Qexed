//! 服务器上下文：各子系统（players/world/entities/plugins/permissions/...）的聚合点。
//! 迁移自 v4 crates/qexed/src/connection/context.rs 的 ServerContext 服务器侧职责。
//!
//! 适配差异（v6）：v4 的 ServerContext 直接持有各管理器的具体类型；
//! v6 这些管理器分散在 qexed_player / qexed_world / qexed_entities / qexed_plugins
//! 等尚为空壳的 crate，因此这里定义 ServerRuntime trait 做能力视图，
//! qexed 本体组装时实现该 trait 注入真实管理器；服务器域自身管理的子系统
//! （warden/audit/content_filter/code_of_conduct/cluster）保留具体类型。

use std::sync::Arc;

use qexed_config::Config;

use crate::cluster_entities::{ClusterEntityController, ClusterPlayers, RegionRouter};
use crate::cluster_rpc::{ClusterEntityRendering, ClusterEntitySpawning};
use crate::code_of_conduct::CodeOfConductTexts;
use crate::config::{
    CodeOfConductConfig, ContentFilterConfig, PlayerAuditConfig, PlaceholdersConfig,
    ServerConfig, WardenConfig, WorldClusterConfig,
};
use crate::audit::PlayerAuditLogger;
use crate::content_filter::ContentFilter;
use crate::error::Result;
use crate::warden::WardenManager;

/// 服务器运行时能力视图（v4 ServerContext 各字段的 trait 化）。
pub trait ServerRuntime: ClusterPlayers + std::fmt::Debug + Send + Sync {
    /// 服务器核心配置。
    fn server_config(&self) -> &ServerConfig;

    /// 全局服务 tick（实体生成/AI/掉落物等）。返回是否正常。
    fn global_service_tick(&self) -> Result<()> {
        Ok(())
    }

    /// 插件摘要（console plugins 命令）。
    fn plugin_summaries(&self) -> Vec<String> {
        Vec::new()
    }

    /// 触发插件配置重载事件（console reload 命令）。
    fn emit_config_reload(&self, _config_path: &str) {}

    /// 给玩家授予全通配权限（console op 命令）。返回授亓名称。
    fn grant_global_wildcard(&self, _uuid: uuid::Uuid, _username: &str) -> Result<()> {
        Err(crate::error::ServerError::msg("permissions not wired"))
    }

    /// 授予后重发命令树（console op 命令）。
    fn refresh_command_tree(&self, _uuid: uuid::Uuid) -> Result<()> {
        Ok(())
    }

    /// 集群实体控制器（未启用集群时 None）。
    fn cluster_entities(&self) -> Option<&ClusterEntityController<RegionRouter>> {
        None
    }

    /// 集群实体渲染/生成参数。
    fn cluster_rendering(&self) -> ClusterEntityRendering {
        ClusterEntityRendering {
            default_distance: 64.0,
            player_distance: 64.0,
            npc_distance: 64.0,
            hologram_distance: 64.0,
            item_distance: 64.0,
            item_merge_radius: 0.5,
            item_merge_max_stack: 64,
            stack_threshold: 0,
            stack_radius: 0.0,
        }
    }

    fn cluster_spawning(&self) -> ClusterEntitySpawning {
        ClusterEntitySpawning {
            enable: false,
            tick_interval_ms: 50,
            global_cap: 0,
            slime_chunks: serde_json::Value::Null,
            rules: Vec::new(),
            per_dimension_cap: 0,
            per_type_cap: 0,
            max_spawn_per_tick: 0,
            player_activation_range: 0.0,
        }
    }

    /// 默认游玩维度。
    fn default_dimension(&self) -> String {
        "minecraft:overworld".to_string()
    }

    /// 模拟距离（chunk）。
    fn simulation_distance(&self) -> i32 {
        8
    }
}

/// 服务器域自身管理的子系统集合（可独立于 ServerRuntime 构造）。
#[derive(Debug, Clone)]
pub struct ServerServices {
    pub warden: Arc<WardenManager>,
    pub player_audit: Arc<PlayerAuditLogger>,
    pub content_filter: Arc<ContentFilter>,
    pub code_of_conducts: Arc<CodeOfConductTexts>,
}

impl ServerServices {
    /// 按配置构造各子系统（config 加载失败时退回禁用态并告警）。
    pub fn load() -> Self {
        let warden = WardenConfig::load_and_create_default(false)
            .map(WardenManager::from_config)
            .unwrap_or_else(|err| {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.warden.load_failed")
                        .replace("%{error}", &err.to_string())
                );
                WardenManager::from_config(WardenConfig::default())
            });
        let player_audit = PlayerAuditConfig::load_and_create_default(false)
            .map(|config| PlayerAuditLogger::from_config(&config))
            .unwrap_or_else(|err| {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.audit.load_failed")
                        .replace("%{error}", &err.to_string())
                );
                PlayerAuditLogger::from_config(&PlayerAuditConfig::default())
            });
        let content_filter = ContentFilterConfig::load_and_create_default(false)
            .map_err(|err| err.to_string())
            .and_then(|config| ContentFilter::from_config(&config).map_err(|err| err.to_string()))
            .unwrap_or_else(|err| {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.content_filter.load_failed")
                        .replace("%{error}", &err)
                );
                ContentFilter::from_config(&ContentFilterConfig::default())
                    .unwrap_or_else(|_| ContentFilter::disabled())
            });
        let code_of_conducts = CodeOfConductConfig::load_and_create_default(false)
            .map_err(|err| err.to_string())
            .and_then(|config| {
                CodeOfConductTexts::load(config.enable, &config.dir).map_err(|err| err.to_string())
            })
            .unwrap_or_else(|err| {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.code_of_conduct.load_failed")
                        .replace("%{error}", &err)
                );
                CodeOfConductTexts::default()
            });
        Self {
            warden: Arc::new(warden),
            player_audit: Arc::new(player_audit),
            content_filter: Arc::new(content_filter),
            code_of_conducts: Arc::new(code_of_conducts),
        }
    }
}

/// 集群实体控制器工厂：按 WorldClusterConfig 构建（禁用或无分片 → None）。
pub fn build_cluster_entities(
    config: &WorldClusterConfig,
) -> Option<ClusterEntityController<RegionRouter>> {
    let router = RegionRouter::from_config(config)?;
    ClusterEntityController::from_config(config, router)
}

/// 占位符上下文快照（console status / placeholders 用）。
pub struct ServerStatusSnapshot {
    pub online_players: usize,
    pub max_players: i32,
}

impl ServerStatusSnapshot {
    pub fn from_config(config: &ServerConfig, online_players: usize) -> Self {
        Self {
            online_players,
            max_players: config.max_player,
        }
    }
}

/// placeholders 开关的加载helper（避免 ServerRuntime 暴露全部配置结构）。
pub fn placeholders_enabled() -> bool {
    PlaceholdersConfig::load_and_create_default(false)
        .map(|config| config.enable)
        .unwrap_or(true)
}
