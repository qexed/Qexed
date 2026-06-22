use qexed_plugin_sdk::{
    CustomEntityDefinition, CustomEntityRegistryResponse, EntityAiEntityPayload,
    EntityAiOperation, EntityAiPlayerPayload, EntityAiTickQuery, EntityAiTickResponse,
    PlayerAction, PlayerPositionPayload, PluginCommandDefinition, PluginCommandQuery,
    PluginCommandResponse, RuntimeEntity, entity_upsert,
};

qexed_plugin_sdk::qexed_plugin_memory!();

const ASSASSIN_ID: &str = "pve:assassin";
const LURER_ID: &str = "pve:lurer";
const ASSASSIN_AI: &str = "plugin:pve_assassin";
const LURER_AI: &str = "plugin:pve_lurer";

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 { 500 }

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("pve_entity v3 — /pve summon <assassin|lurer>");
}

// ── /pve command ──

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: "pve".to_string(),
        description_key: "pve.entity.summon".to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PluginCommandQuery>(ptr, len) })
    else { return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default()) };
    if payload.command != "pve" { return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default()) }

    let args: Vec<&str> = payload.argument.split_whitespace().collect();
    if args.len() < 2 || args[0] != "summon" {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true, actions: vec![PlayerAction::SystemMessage {
                text: "§c用法: /pve summon <assassin|lurer>".into(), translate: String::new(), with: vec![], overlay: false,
            }],
        });
    }

    let (entity_type, display_name, ai_key, ai_params) = match args[1] {
        "assassin" => ("minecraft:zombie","§4刺杀者", ASSASSIN_AI,
            r#"{"vanilla_behavior":"hostile_melee","movement_speed":0.35,"follow_range":32,"tick_interval_ms":200,"attack_damage":4.0,"max_health":30}"#),
        "lurer" => ("minecraft:iron_golem","§6诱引者", LURER_AI,
            r#"{"vanilla_behavior":"hostile_melee","movement_speed":0.20,"follow_range":32,"tick_interval_ms":200,"attack_damage":6.0,"max_health":60}"#),
        _ => return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true, actions: vec![PlayerAction::SystemMessage {
                text: "§c未知类型，可用: assassin, lurer".into(), translate: String::new(), with: vec![], overlay: false,
            }],
        }),
    };

    let key = format!("pve_{}_{}", args[1], qexed_plugin_sdk::time_millis());
    let spawned = entity_upsert(&RuntimeEntity {
        key: &key, dimension: &payload.player.dimension, entity_type,
        x: 2.0, y: -58.0, z: 2.0, yaw: 0.0, pitch: 0.0,
        display_name, ai: ai_key, ai_params_json: ai_params, auto_jump: true,
    });

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true, actions: vec![PlayerAction::SystemMessage {
            text: if spawned { format!("§a已召唤 {}", display_name) } else { format!("§c召唤失败") },
            translate: String::new(), with: vec![], overlay: false,
        }],
    })
}

// ── Custom entities ──

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_custom_entities(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&CustomEntityRegistryResponse {
        entities: vec![
            CustomEntityDefinition {
                id: ASSASSIN_ID.into(), entity_type: "minecraft:zombie".into(),
                shell_entity_type: "minecraft:zombie".into(), registry_id: None,
                display_name: r#"{"text":"刺杀者","color":"dark_red","bold":true}"#.into(),
                ai: ASSASSIN_AI.into(),
                ai_params: [
                    ("vanilla_behavior".into(), serde_json::json!("hostile_melee")),
                    ("movement_speed".into(), serde_json::json!(0.35)),
                    ("follow_range".into(), serde_json::json!(32.0)),
                    ("tick_interval_ms".into(), serde_json::json!(200)),
                    ("attack_damage".into(), serde_json::json!(4.0)),
                    ("max_health".into(), serde_json::json!(30.0)),
                ].into_iter().collect(),
            },
            CustomEntityDefinition {
                id: LURER_ID.into(), entity_type: "minecraft:zombie".into(),
                shell_entity_type: "minecraft:iron_golem".into(), registry_id: None,
                display_name: r#"{"text":"诱引者","color":"gold","bold":true}"#.into(),
                ai: LURER_AI.into(),
                ai_params: [
                    ("vanilla_behavior".into(), serde_json::json!("hostile_melee")),
                    ("movement_speed".into(), serde_json::json!(0.20)),
                    ("follow_range".into(), serde_json::json!(32.0)),
                    ("tick_interval_ms".into(), serde_json::json!(200)),
                    ("attack_damage".into(), serde_json::json!(6.0)),
                    ("max_health".into(), serde_json::json!(60.0)),
                ].into_iter().collect(),
            },
        ],
    })
}

// ── Entity AI tick ──

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_entity_ai_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<EntityAiTickQuery>(ptr, len) })
    else { return qexed_plugin_sdk::response_ptr_len(&EntityAiTickResponse::default()) };

    match payload.entity.ai.as_str() {
        ASSASSIN_AI => qexed_plugin_sdk::response_ptr_len(&assassin_ai(&payload)),
        LURER_AI => qexed_plugin_sdk::response_ptr_len(&lurer_ai(&payload)),
        _ => qexed_plugin_sdk::response_ptr_len(&EntityAiTickResponse::default()),
    }
}

fn assassin_ai(query: &EntityAiTickQuery) -> EntityAiTickResponse {
    let e = &query.entity;
    let fast = param_f64(e, "movement_speed", 0.35);
    let slow = fast * 0.25;
    let angle = param_f64(e, "look_detection_angle", 55.0);
    let Some(n) = nearest(e, &query.nearby_players) else { return EntityAiTickResponse::default() };
    let watched = any_looking(e, &query.nearby_players, angle);
    let speed = if watched { slow } else { fast };
    let (dx, dz) = toward(e, &n.position, speed);
    EntityAiTickResponse { operations: vec![
        EntityAiOperation::MoveDelta { x: dx, y: 0.0, z: dz, yaw: None, pitch: None },
        EntityAiOperation::LookAt { x: n.position.x, y: n.position.y + 1.6, z: n.position.z },
    ]}
}

fn lurer_ai(query: &EntityAiTickQuery) -> EntityAiTickResponse {
    let e = &query.entity;
    let speed = param_f64(e, "movement_speed", 0.20);
    let Some(n) = nearest(e, &query.nearby_players) else { return EntityAiTickResponse::default() };
    let (dx, dz) = toward(e, &n.position, speed);
    EntityAiTickResponse { operations: vec![
        EntityAiOperation::MoveDelta { x: dx, y: 0.0, z: dz, yaw: None, pitch: None },
        EntityAiOperation::LookAt { x: n.position.x, y: n.position.y + 1.6, z: n.position.z },
    ]}
}

// ── helpers ──

fn nearest(e: &EntityAiEntityPayload, players: &[EntityAiPlayerPayload]) -> Option<EntityAiPlayerPayload> {
    players.iter().min_by(|a,b| {
        let da = (a.position.x-e.position.x).powi(2)+(a.position.y-e.position.y).powi(2)+(a.position.z-e.position.z).powi(2);
        let db = (b.position.x-e.position.x).powi(2)+(b.position.y-e.position.y).powi(2)+(b.position.z-e.position.z).powi(2);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    }).cloned()
}

fn toward(e: &EntityAiEntityPayload, t: &PlayerPositionPayload, speed: f64) -> (f64, f64) {
    let dx = t.x - e.position.x;
    let dz = t.z - e.position.z;
    let d = (dx*dx+dz*dz).sqrt().max(0.001);
    (dx/d*speed, dz/d*speed)
}

fn any_looking(e: &EntityAiEntityPayload, players: &[EntityAiPlayerPayload], deg: f64) -> bool {
    let half = deg.to_radians()/2.0;
    for p in players {
        let dx = e.position.x - p.position.x;
        let dy = (e.position.y + 1.0) - (p.position.y + 1.62);
        let dz = e.position.z - p.position.z;
        let dist = (dx*dx+dz*dz).sqrt();
        if dist < 0.5 { return true }
        let a = (-dx.atan2(dz)).rem_euclid(std::f64::consts::TAU);
        let y = (p.position.yaw as f64).to_radians().rem_euclid(std::f64::consts::TAU);
        let mut diff = (a - y).rem_euclid(std::f64::consts::TAU);
        if diff > std::f64::consts::PI { diff = std::f64::consts::TAU - diff }
        if diff < half { return true }
    }
    false
}

fn param_f64(e: &EntityAiEntityPayload, k: &str, d: f64) -> f64 {
    e.ai_params.get(k).and_then(|v| v.as_f64()).unwrap_or(d)
}
