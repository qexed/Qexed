use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct DimensionWorldRules {
    #[serde(default = "default_dimension")]
    #[AutoDoc(key = "config.qexed.server.world.dimension")]
    pub dimension: String,

    #[serde(default = "default_dimension_type")]
    #[AutoDoc(key = "config.qexed.server.world.dimension_type")]
    pub dimension_type: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.read_only")]
    pub read_only: bool,

    #[serde(default = "default_block_updates")]
    #[AutoDoc(key = "config.qexed.server.world.rules.block_updates")]
    pub block_updates: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light")]
    pub light: super::server::LightMode,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light_algorithm")]
    pub light_algorithm: super::server::LightAlgorithm,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.rules.time", sub)]
    pub time: DimensionTimeRule,
}

impl Default for DimensionWorldRules {
    fn default() -> Self {
        Self {
            dimension: default_dimension(),
            dimension_type: default_dimension_type(),
            read_only: false,
            block_updates: default_block_updates(),
            light: super::server::LightMode::default(),
            light_algorithm: super::server::LightAlgorithm::default(),
            time: DimensionTimeRule::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct DimensionTimeRule {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.rules.time.value")]
    pub value: i64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.rules.time.fixed")]
    pub fixed: Option<i64>,

    #[serde(default = "default_daylight_cycle")]
    #[AutoDoc(key = "config.qexed.server.world.rules.time.daylight_cycle")]
    pub daylight_cycle: bool,

    #[serde(default = "default_tick_step")]
    #[AutoDoc(key = "config.qexed.server.world.rules.time.tick_step")]
    pub tick_step: i64,
}

impl Default for DimensionTimeRule {
    fn default() -> Self {
        Self {
            value: 0,
            fixed: None,
            daylight_cycle: default_daylight_cycle(),
            tick_step: default_tick_step(),
        }
    }
}

fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_dimension_type() -> String {
    "minecraft:overworld".to_string()
}

fn default_block_updates() -> bool {
    true
}

fn default_daylight_cycle() -> bool {
    true
}

fn default_tick_step() -> i64 {
    1
}

impl qexed_config::tool::AppConfigTrait for DimensionWorldRules {
    const PATH: &'static str = "/";
    const NAME: &'static str = "rules";
}
