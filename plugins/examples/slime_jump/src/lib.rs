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
#[cfg(not(test))]
const LEGACY_TRIGGER_BLOCK: &str = "minecraft:slime_block";
const GRAVITY_PER_TICK: f64 = 0.08;
const MIN_ARC_HEIGHT: f64 = 0.5;
const MIN_FLIGHT_TICKS: f64 = 1.0;
const DEFAULT_HORIZONTAL_MULTIPLIER: f64 = 5.0;
const DEFAULT_VERTICAL_MULTIPLIER: f64 = 1.0;
const DEFAULT_MAX_HORIZONTAL_SPEED: f64 = 16.0;

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
    if !config.is_trigger_block(&payload.block_name) {
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
        JumpTuning {
            max_height: config.max_height,
            horizontal_multiplier: config.horizontal_multiplier,
            vertical_multiplier: config.vertical_multiplier,
            max_horizontal_speed: config.max_horizontal_speed,
        },
    )
}

fn ballistic_velocity(start: Point, target: Point, tuning: JumpTuning) -> Option<Velocity> {
    let arc_height = finite_or(tuning.max_height, MIN_ARC_HEIGHT).max(MIN_ARC_HEIGHT);
    let apex_y = start.y.max(target.y) + arc_height;
    if !apex_y.is_finite() || !start.is_finite() || !target.is_finite() {
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

    let horizontal_multiplier =
        finite_or(tuning.horizontal_multiplier, DEFAULT_HORIZONTAL_MULTIPLIER).max(0.0);
    let vertical_multiplier =
        finite_or(tuning.vertical_multiplier, DEFAULT_VERTICAL_MULTIPLIER).max(0.0);
    let max_horizontal_speed =
        finite_or(tuning.max_horizontal_speed, DEFAULT_MAX_HORIZONTAL_SPEED).max(0.0);
    let mut x = (target.x - start.x) / flight_ticks * horizontal_multiplier;
    let mut z = (target.z - start.z) / flight_ticks * horizontal_multiplier;
    clamp_horizontal_speed(&mut x, &mut z, max_horizontal_speed);

    Some(Velocity {
        x,
        y: vertical_velocity * vertical_multiplier,
        z,
    })
}

fn finite_or(value: f64, default: f64) -> f64 {
    if value.is_finite() { value } else { default }
}

fn clamp_horizontal_speed(x: &mut f64, z: &mut f64, max_speed: f64) {
    let speed = x.hypot(*z);
    if !speed.is_finite() || speed <= max_speed || speed <= 0.0 {
        return;
    }
    let scale = max_speed / speed;
    *x *= scale;
    *z *= scale;
}

#[derive(Debug, Clone, Copy)]
struct JumpTuning {
    max_height: f64,
    horizontal_multiplier: f64,
    vertical_multiplier: f64,
    max_horizontal_speed: f64,
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
    #[serde(default = "default_trigger_blocks")]
    trigger_blocks: Vec<String>,
    #[serde(default = "default_max_height")]
    max_height: f64,
    #[serde(default = "default_horizontal_multiplier")]
    horizontal_multiplier: f64,
    #[serde(default = "default_vertical_multiplier")]
    vertical_multiplier: f64,
    #[serde(default = "default_max_horizontal_speed")]
    max_horizontal_speed: f64,
    #[serde(default)]
    target: TargetConfig,
}

#[cfg(not(test))]
impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            trigger_block: default_trigger_block(),
            trigger_blocks: default_trigger_blocks(),
            max_height: default_max_height(),
            horizontal_multiplier: default_horizontal_multiplier(),
            vertical_multiplier: default_vertical_multiplier(),
            max_horizontal_speed: default_max_horizontal_speed(),
            target: TargetConfig::default(),
        }
    }
}

#[cfg(not(test))]
impl Config {
    fn is_trigger_block(&self, block_name: &str) -> bool {
        block_name == self.trigger_block
            || self.trigger_blocks.iter().any(|name| name == block_name)
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
fn default_trigger_blocks() -> Vec<String> {
    vec![
        DEFAULT_TRIGGER_BLOCK.to_string(),
        LEGACY_TRIGGER_BLOCK.to_string(),
    ]
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
fn default_horizontal_multiplier() -> f64 {
    DEFAULT_HORIZONTAL_MULTIPLIER
}

#[cfg(not(test))]
fn default_vertical_multiplier() -> f64 {
    DEFAULT_VERTICAL_MULTIPLIER
}

#[cfg(not(test))]
fn default_max_horizontal_speed() -> f64 {
    DEFAULT_MAX_HORIZONTAL_SPEED
}

#[cfg(not(test))]
const DEFAULT_CONFIG: &str = r#"enable = true
# Gold pressure plate; slime block remains as a legacy fallback.
trigger_blocks = [
    "minecraft:light_weighted_pressure_plate",
    "minecraft:slime_block",
]
max_height = 8.0
# Horizontal distance is damped heavily by the client, so long jumps need a boost.
horizontal_multiplier = 5.0
vertical_multiplier = 1.0
max_horizontal_speed = 16.0

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
            JumpTuning {
                max_height: 8.0,
                horizontal_multiplier: 1.0,
                vertical_multiplier: 1.0,
                max_horizontal_speed: 16.0,
            },
        )
        .expect("reachable target");

        assert!(velocity.x > 0.0);
        assert!(velocity.y > 0.0);
        assert!(velocity.z > 0.0);
    }

    #[test]
    fn target_above_start_is_reachable() {
        let velocity = ballistic_velocity(
            Point {
                x: 0.0,
                y: 64.0,
                z: 0.0,
            },
            Point {
                x: 4.0,
                y: 80.0,
                z: 0.0,
            },
            JumpTuning {
                max_height: 8.0,
                horizontal_multiplier: 1.0,
                vertical_multiplier: 1.0,
                max_horizontal_speed: 16.0,
            },
        )
        .expect("target above start");

        assert!(velocity.x > 0.0);
        assert!(velocity.y > 0.0);
    }

    #[test]
    fn horizontal_multiplier_increases_launch_speed() {
        let base = ballistic_velocity(
            Point {
                x: 0.0,
                y: 64.0,
                z: 0.0,
            },
            Point {
                x: 80.0,
                y: 64.0,
                z: 0.0,
            },
            JumpTuning {
                max_height: 8.0,
                horizontal_multiplier: 1.0,
                vertical_multiplier: 1.0,
                max_horizontal_speed: 16.0,
            },
        )
        .expect("base velocity");
        let boosted = ballistic_velocity(
            Point {
                x: 0.0,
                y: 64.0,
                z: 0.0,
            },
            Point {
                x: 80.0,
                y: 64.0,
                z: 0.0,
            },
            JumpTuning {
                max_height: 8.0,
                horizontal_multiplier: 5.0,
                vertical_multiplier: 1.0,
                max_horizontal_speed: 16.0,
            },
        )
        .expect("boosted velocity");

        assert!((boosted.x - base.x * 5.0).abs() < 1e-9);
    }

    #[test]
    fn horizontal_speed_is_clamped() {
        let velocity = ballistic_velocity(
            Point {
                x: 0.0,
                y: 64.0,
                z: 0.0,
            },
            Point {
                x: 100.0,
                y: 64.0,
                z: 100.0,
            },
            JumpTuning {
                max_height: 8.0,
                horizontal_multiplier: 5.0,
                vertical_multiplier: 1.0,
                max_horizontal_speed: 10.0,
            },
        )
        .expect("clamped velocity");

        assert!((velocity.x.hypot(velocity.z) - 10.0).abs() < 1e-9);
    }
}
