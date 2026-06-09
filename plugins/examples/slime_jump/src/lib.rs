#[cfg(not(test))]
use qexed_plugin_sdk::{
    BlockStepPayload, PlayerAction, PluginCommandResponse, config_load_or_create,
    config_read_to_string,
};
#[cfg(not(test))]
use serde::Deserialize;

#[cfg(not(test))]
qexed_plugin_sdk::qexed_plugin_memory!();

#[cfg(not(test))]
const CONFIG_PATH: &str = "config.toml";
#[cfg(not(test))]
const DEFAULT_TRIGGER_BLOCK: &str = "minecraft:light_weighted_pressure_plate";
const GRAVITY_PER_TICK: f64 = 0.08;
const MIN_ARC_HEIGHT: f64 = 0.5;
const MIN_FLIGHT_TICKS: f64 = 1.0;

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    100
}

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    qexed_plugin_sdk::log("slime_jump initialized");
}

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_block_step(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockStepPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let config = load_config();
    if !config.enable || payload.dimension != config.target.dimension {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }
    if payload.block_name != config.trigger_block {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let Some(velocity) = jump_velocity(&payload, &config) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::Velocity {
            x: velocity.x,
            y: velocity.y,
            z: velocity.z,
            additive: false,
        }],
    })
}

#[cfg(not(test))]
fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str::<Config>(&contents).ok())
        .unwrap_or_default()
}

#[cfg(not(test))]
fn jump_velocity(payload: &BlockStepPayload, config: &Config) -> Option<Velocity> {
    ballistic_velocity(
        Point {
            x: payload.player_position.x,
            y: payload.player_position.y,
            z: payload.player_position.z,
        },
        Point {
            x: config.target.x,
            y: config.target.y,
            z: config.target.z,
        },
        config.max_height,
    )
}

fn ballistic_velocity(start: Point, target: Point, max_height: f64) -> Option<Velocity> {
    let arc_height = max_height.max(MIN_ARC_HEIGHT);
    let apex_y = start.y + arc_height;
    if apex_y < target.y || !start.is_finite() || !target.is_finite() {
        return None;
    }

    let up_distance = (apex_y - start.y).max(0.0);
    let down_distance = (apex_y - target.y).max(0.0);
    let vertical_velocity = (2.0 * GRAVITY_PER_TICK * up_distance).sqrt();
    let ticks_up = vertical_velocity / GRAVITY_PER_TICK;
    let ticks_down = (2.0 * down_distance / GRAVITY_PER_TICK).sqrt();
    let flight_ticks = ticks_up + ticks_down;
    if !flight_ticks.is_finite() || flight_ticks < MIN_FLIGHT_TICKS {
        return None;
    }

    Some(Velocity {
        x: (target.x - start.x) / flight_ticks,
        y: vertical_velocity,
        z: (target.z - start.z) / flight_ticks,
    })
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}

impl Point {
    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Debug, Clone, Copy)]
struct Velocity {
    x: f64,
    y: f64,
    z: f64,
}

#[cfg(not(test))]
#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_trigger_block")]
    trigger_block: String,
    #[serde(default = "default_max_height")]
    max_height: f64,
    #[serde(default)]
    target: TargetConfig,
}

#[cfg(not(test))]
impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            trigger_block: default_trigger_block(),
            max_height: default_max_height(),
            target: TargetConfig::default(),
        }
    }
}

#[cfg(not(test))]
#[derive(Debug, Clone, Deserialize)]
struct TargetConfig {
    #[serde(default = "default_dimension")]
    dimension: String,
    #[serde(default = "default_target_x")]
    x: f64,
    #[serde(default = "default_target_y")]
    y: f64,
    #[serde(default = "default_target_z")]
    z: f64,
}

#[cfg(not(test))]
impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            dimension: default_dimension(),
            x: default_target_x(),
            y: default_target_y(),
            z: default_target_z(),
        }
    }
}

#[cfg(not(test))]
fn default_true() -> bool {
    true
}

#[cfg(not(test))]
fn default_trigger_block() -> String {
    DEFAULT_TRIGGER_BLOCK.to_string()
}

#[cfg(not(test))]
fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}

#[cfg(not(test))]
fn default_target_x() -> f64 {
    0.5
}

#[cfg(not(test))]
fn default_target_y() -> f64 {
    80.0
}

#[cfg(not(test))]
fn default_target_z() -> f64 {
    0.5
}

#[cfg(not(test))]
fn default_max_height() -> f64 {
    8.0
}

#[cfg(not(test))]
const DEFAULT_CONFIG: &str = r#"enable = true
trigger_block = "minecraft:light_weighted_pressure_plate"
max_height = 8.0

[target]
dimension = "minecraft:overworld"
x = 0.5
y = 80.0
z = 0.5
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_points_towards_target() {
        let velocity = ballistic_velocity(
            Point {
                x: 0.5,
                y: 64.0,
                z: 0.5,
            },
            Point {
                x: 16.5,
                y: 64.0,
                z: 8.5,
            },
            8.0,
        )
        .expect("reachable target");

        assert!(velocity.x > 0.0);
        assert!(velocity.y > 0.0);
        assert!(velocity.z > 0.0);
    }

    #[test]
    fn target_above_max_height_is_rejected() {
        assert!(
            ballistic_velocity(
                Point {
                    x: 0.0,
                    y: 64.0,
                    z: 0.0,
                },
                Point {
                    x: 0.0,
                    y: 80.0,
                    z: 0.0,
                },
                8.0,
            )
            .is_none()
        );
    }
}
