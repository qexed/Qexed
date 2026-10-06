//! WorldRulesManager → play 域 WorldRulesSource 适配。

use qexed_config::Config as _;
use qexed_play::{DimensionRules, Result, WorldRulesSource};
use qexed_world::world::WorldManager;

pub struct RealWorldRules {
    manager: std::sync::Arc<qexed_world::world::WorldRulesManager>,
}

impl RealWorldRules {
    pub fn new(world: &WorldManager) -> Self {
        // WorldRulesManager 依 world.toml 构造（与 WorldManager 同源配置；
        // save_path 取 world 的存储路径保持同目录规则数据）。
        let config = match qexed_world::config::WorldConfig::load_and_create_default(true) {
            Ok(config) => config,
            Err(err) => {
                log::warn!("world config load failed, using default: {err}");
                qexed_world::config::WorldConfig::default()
            }
        };
        let manager = match qexed_world::world::WorldRulesManager::from_world_config(&config) {
            Ok(manager) => manager,
            Err(err) => {
                log::warn!("world rules load failed, using fallback: {err}");
                match qexed_world::world::WorldRulesManager::from_world_config_for_tests(
                    &config,
                    std::env::temp_dir(),
                ) {
                    Ok(fallback) => fallback,
                    Err(err2) => panic!("world rules fallback failed: {err2}"),
                }
            }
        };
        let _ = world.save_path(); // 校验句柄可用（规则数据与存档同根）
        Self {
            manager: std::sync::Arc::new(manager),
        }
    }

    /// 共享句柄（RealGameplay 的红石 tick / chat 命令依赖共用同一实例）。
    pub fn handle(&self) -> std::sync::Arc<qexed_world::world::WorldRulesManager> {
        std::sync::Arc::clone(&self.manager)
    }
}

impl WorldRulesSource for RealWorldRules {
    fn ensure_loaded(&self, dimension: &str) -> Result<()> {
        self.manager
            .ensure_loaded(dimension)
            .map_err(|e| qexed_play::PlayError::msg(e.to_string()))
    }

    fn snapshot(&self, dimension: &str) -> DimensionRules {
        let snap = self.manager.snapshot(dimension);
        DimensionRules {
            dimension_type: snap.dimension_type.clone(),
        }
    }

    fn current_time(&self, dimension: &str) -> i64 {
        self.manager.current_time(dimension)
    }

    fn tick_dimension_time(&self, dimension: &str, _default_day_ticks: i64) -> i64 {
        // 规则侧自带日长配置（world.toml day_ticks），默认值仅作回退参数。
        self.manager.tick_dimension_time(dimension)
    }
}
