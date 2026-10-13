//! pve_entity：v4 示例移植 —— /pve summon 召唤带 AI 的实体。
//!
//! v4：命令返回动作 + custom_entities / entity_ai_tick 导出函数。
//! v6：ChatCommandEvent（cancel 接管命令）+ EntityAiTickEvent（Mutex response 回写）。
//! v4 的 custom_entities 注册在 v6 宿主尚未接线（休眠），召唤走 entity_upsert。

use qexed_plugin_api::payload::{
    EntityAiEntityPayload, EntityAiOperation, EntityAiPlayerPayload, EntityAiTickQuery,
    PlayerPositionPayload,
};
use qexed_plugin_api::services::RuntimeEntitySpec;
use qexed_plugin_api::{ChatCommandEvent, EntityAiTickEvent, EventContext, LogLevel};
use qexed_plugin_sdk::{InitFuture, PluginContext};

const ASSASSIN_AI: &str = "plugin:pve_assassin";
const LURER_AI: &str = "plugin:pve_lurer";

const ASSASSIN_PARAMS: &str = r#"{"vanilla_behavior":"hostile_melee","movement_speed":0.35,"follow_range":32,"tick_interval_ms":200,"attack_damage":4.0,"max_health":30}"#;
const LURER_PARAMS: &str = r#"{"vanilla_behavior":"hostile_melee","movement_speed":0.20,"follow_range":32,"tick_interval_ms":200,"attack_damage":6.0,"max_health":60}"#;

qexed_plugin_sdk::declare_simple_plugin!("pve_entity", "PvE Entity", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        ctx.register_command("pve");

        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != "pve" {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let args: Vec<&str> = ev.argument.split_whitespace().collect();
                let reply = |text: String| {
                    sdk.perform(&ev.player, qexed_plugin_sdk::system_message(text));
                };
                if args.len() < 2 || args[0] != "summon" {
                    reply("§c用法: /pve summon <assassin|lurer>".to_string());
                    return;
                }
                let (entity_type, display_name, ai_key, ai_params) = match args[1] {
                    "assassin" => ("minecraft:zombie", "§4刺杀者", ASSASSIN_AI, ASSASSIN_PARAMS),
                    "lurer" => ("minecraft:iron_golem", "§6诱引者", LURER_AI, LURER_PARAMS),
                    _ => {
                        reply("§c未知类型，可用: assassin, lurer".to_string());
                        return;
                    }
                };
                let key = format!("pve_{}_{}", args[1], now_millis());
                let spawned = sdk.entity_upsert(&RuntimeEntitySpec {
                    key,
                    dimension: ev.player.dimension.clone(),
                    entity_type: entity_type.to_string(),
                    x: 2.0,
                    y: -58.0,
                    z: 2.0,
                    yaw: 0.0,
                    pitch: 0.0,
                    display_name: display_name.to_string(),
                    ai: ai_key.to_string(),
                    ai_params_json: ai_params.to_string(),
                    auto_jump: true,
                });
                reply(if spawned {
                    format!("§a已召唤 {display_name}")
                } else {
                    "§c召唤失败".to_string()
                });
            })
        });

        ctx.register::<EntityAiTickEvent>(|_api, ev, _| {
            Box::pin(async move {
                let ops = match ev.query.entity.ai.as_str() {
                    ASSASSIN_AI => assassin_ai(&ev.query),
                    LURER_AI => lurer_ai(&ev.query),
                    _ => return,
                };
                if !ops.is_empty() {
                    ev.response.lock().unwrap().operations = ops;
                }
            })
        });

        ctx.log(LogLevel::Info, "pve_entity v3 — /pve summon <assassin|lurer>");
    })
});

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn assassin_ai(query: &EntityAiTickQuery) -> Vec<EntityAiOperation> {
    let e = &query.entity;
    let fast = param_f64(e, "movement_speed", 0.35);
    let slow = fast * 0.25;
    let angle = param_f64(e, "look_detection_angle", 55.0);
    let Some(n) = nearest(e, &query.nearby_players) else {
        return Vec::new();
    };
    let watched = any_looking(e, &query.nearby_players, angle);
    let speed = if watched { slow } else { fast };
    let (dx, dz) = toward(e, &n.position, speed);
    vec![
        EntityAiOperation::MoveDelta { x: dx, y: 0.0, z: dz, yaw: None, pitch: None },
        EntityAiOperation::LookAt { x: n.position.x, y: n.position.y + 1.6, z: n.position.z },
    ]
}

fn lurer_ai(query: &EntityAiTickQuery) -> Vec<EntityAiOperation> {
    let e = &query.entity;
    let speed = param_f64(e, "movement_speed", 0.20);
    let Some(n) = nearest(e, &query.nearby_players) else {
        return Vec::new();
    };
    let (dx, dz) = toward(e, &n.position, speed);
    vec![
        EntityAiOperation::MoveDelta { x: dx, y: 0.0, z: dz, yaw: None, pitch: None },
        EntityAiOperation::LookAt { x: n.position.x, y: n.position.y + 1.6, z: n.position.z },
    ]
}

fn nearest(e: &EntityAiEntityPayload, players: &[EntityAiPlayerPayload]) -> Option<EntityAiPlayerPayload> {
    players
        .iter()
        .min_by(|a, b| {
            let da = (a.position.x - e.position.x).powi(2)
                + (a.position.y - e.position.y).powi(2)
                + (a.position.z - e.position.z).powi(2);
            let db = (b.position.x - e.position.x).powi(2)
                + (b.position.y - e.position.y).powi(2)
                + (b.position.z - e.position.z).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn toward(e: &EntityAiEntityPayload, t: &PlayerPositionPayload, speed: f64) -> (f64, f64) {
    let dx = t.x - e.position.x;
    let dz = t.z - e.position.z;
    let d = (dx * dx + dz * dz).sqrt().max(0.001);
    (dx / d * speed, dz / d * speed)
}

fn any_looking(e: &EntityAiEntityPayload, players: &[EntityAiPlayerPayload], deg: f64) -> bool {
    let half = deg.to_radians() / 2.0;
    for p in players {
        let dx = e.position.x - p.position.x;
        let dy = (e.position.y + 1.0) - (p.position.y + 1.62);
        let _ = dy;
        let dz = e.position.z - p.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < 0.5 {
            return true;
        }
        let a = (-dx.atan2(dz)).rem_euclid(std::f64::consts::TAU);
        let y = f64::from(p.position.yaw).to_radians().rem_euclid(std::f64::consts::TAU);
        let mut diff = (a - y).rem_euclid(std::f64::consts::TAU);
        if diff > std::f64::consts::PI {
            diff = std::f64::consts::TAU - diff;
        }
        if diff < half {
            return true;
        }
    }
    false
}

fn param_f64(e: &EntityAiEntityPayload, k: &str, d: f64) -> f64 {
    e.ai_params.get(k).and_then(|v| v.as_f64()).unwrap_or(d)
}
