//! slime_jump：v4 移植 —— 踩上触发方块弹射至目标点的弹道跳跃。
//!
//! v4：player_block_step 导出，按 block_name 过滤触发方块（压力板/粘液块）。
//! v6：PlayerBlockStepEvent 目前只给坐标（无方块名），故以 `trigger_positions`
//!     坐标过滤替代；为空时维度内任意踩踏触发（v4 语义退化，注释保留）。
//! 弹道算法与 v4 完全一致：g=0.08/tick，顶点取起终点较高者 + max_height。

use qexed_plugin_api::payload::PlayerAction as Pa;
use qexed_plugin_api::{LogLevel, PlayerBlockStepEvent};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::Deserialize;

const GRAVITY_PER_TICK: f64 = 0.08;
const MIN_ARC_HEIGHT: f64 = 0.5;
const MIN_FLIGHT_TICKS: f64 = 1.0;
const DEFAULT_HORIZONTAL_MULTIPLIER: f64 = 5.0;
const DEFAULT_VERTICAL_MULTIPLIER: f64 = 1.0;
const DEFAULT_MAX_HORIZONTAL_SPEED: f64 = 16.0;

qexed_plugin_sdk::declare_simple_plugin!("slime_jump", "Slime Jump", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        ctx.log(LogLevel::Info, "slime_jump initialized");

        ctx.register::<PlayerBlockStepEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable || ev.dimension != config.target.dimension {
                    return;
                }
                // v6 事件无方块名：trigger_positions 非空时按坐标过滤
                if !config.trigger_positions.is_empty() {
                    let hit = config.trigger_positions.iter().any(|p| {
                        p[0] == ev.block.x && p[1] == ev.block.y && p[2] == ev.block.z
                    });
                    if !hit {
                        return;
                    }
                }
                let Some(velocity) = jump_velocity(
                    ev.position.x,
                    ev.position.y,
                    ev.position.z,
                    &config,
                ) else {
                    return;
                };
                sdk.perform(&ev.player, Pa::Velocity {
                    x: velocity.x,
                    y: velocity.y,
                    z: velocity.z,
                    additive: false,
                });
            })
        });
    })
});

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

fn jump_velocity(px: f64, py: f64, pz: f64, config: &Config) -> Option<Velocity> {
    ballistic_velocity(
        Point { x: px, y: py, z: pz },
        Point { x: config.target.x, y: config.target.y, z: config.target.z },
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

    Some(Velocity { x, y: vertical_velocity * vertical_multiplier, z })
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

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default)]
    trigger_positions: Vec<[i32; 3]>,
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
impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            trigger_positions: Vec::new(),
            max_height: default_max_height(),
            horizontal_multiplier: default_horizontal_multiplier(),
            vertical_multiplier: default_vertical_multiplier(),
            max_horizontal_speed: default_max_horizontal_speed(),
            target: TargetConfig::default(),
        }
    }
}

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

fn default_true() -> bool { true }
fn default_dimension() -> String { "minecraft:overworld".to_string() }
fn default_target_x() -> f64 { 0.5 }
fn default_target_y() -> f64 { 80.0 }
fn default_target_z() -> f64 { 0.5 }
fn default_max_height() -> f64 { 8.0 }
fn default_horizontal_multiplier() -> f64 { DEFAULT_HORIZONTAL_MULTIPLIER }
fn default_vertical_multiplier() -> f64 { DEFAULT_VERTICAL_MULTIPLIER }
fn default_max_horizontal_speed() -> f64 { DEFAULT_MAX_HORIZONTAL_SPEED }

fn load_config(sdk: &qexed_plugin_sdk::Sdk) -> Config {
    sdk.read_config()
        .ok()
        .flatten()
        .and_then(|content| {
            content
                .parse::<toml_edit::DocumentMut>()
                .ok()
                .and_then(|doc| qexed_toml::from_document::<Config>(doc).ok())
        })
        .unwrap_or_default()
}

const DEFAULT_CONFIG: &str = r#"enable = true
# v6 事件暂无方块名：这里用坐标过滤触发点（留空 = 维度内任意踩踏触发）。
# 示例：金压力板在 (3, 80, -2)
trigger_positions = []

max_height = 8.0
# 水平距离被客户端强烈衰减，远跳需要加成。
horizontal_multiplier = 5.0
vertical_multiplier = 1.0
max_horizontal_speed = 16.0

[target]
dimension = "minecraft:overworld"
x = 0.5
y = 80.0
z = 0.5
"#;
