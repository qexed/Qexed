use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionWorldRules {
    #[serde(default = "default_dimension")]
    pub dimension: String,

    #[serde(default = "default_dimension_type")]
    pub dimension_type: String,

    #[serde(default)]
    pub read_only: bool,

    #[serde(default = "default_block_updates")]
    pub block_updates: bool,

    #[serde(default)]
    pub light: super::server::LightMode,

    #[serde(default)]
    pub light_algorithm: super::server::LightAlgorithm,

    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionTimeRule {
    #[serde(default)]
    pub value: i64,

    #[serde(default)]
    pub fixed: Option<i64>,

    #[serde(default = "default_daylight_cycle")]
    pub daylight_cycle: bool,

    #[serde(default = "default_tick_step")]
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
