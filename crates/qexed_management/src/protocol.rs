//! JSON-RPC 2.0 线协议与方法分发。
//!
//! 请求：{"jsonrpc":"2.0","id":"...","method":"minecraft:<ns>/<path>","params":{...}}
//! 响应：{"jsonrpc":"2.0","id":"...","result":{...}} 或 {"error":{code,message,data}}
//! 通知：{"jsonrpc":"2.0","method":"minecraft:notification/<domain>/<event>","params":{...}}

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::backend::ManagementBackend;
use crate::error::rpc_code;
use crate::model::{
    AllowlistEntry, Difficulty, GameMode, IpBan, KickRequest, OperatorEntry, PlayerRef,
    SystemMessage, UntypedGameRule, UserBan,
};
use crate::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }

    pub fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0", id, result: None, error: Some(RpcError { code, message: message.into(), data: None }) }
    }
}

/// 处理单条请求（方法分发核心）。
pub fn dispatch(backend: &dyn ManagementBackend, request: &RpcRequest) -> RpcResponse {
    let id = request.id.clone().unwrap_or(Value::Null);
    if request.jsonrpc != "2.0" {
        return RpcResponse::err(id, rpc_code::INVALID_REQUEST, "jsonrpc must be 2.0");
    }
    let params = request.params.clone().unwrap_or(Value::Null);
    match handle_method(backend, &request.method, &params) {
        Ok(result) => RpcResponse::ok(id, result),
        Err(err) => {
            let (code, message) = match &err {
                crate::ManagementError::Message(m) if m.starts_with("method not wired") => {
                    (rpc_code::INTERNAL_ERROR, m.clone())
                }
                crate::ManagementError::Json(e) => {
                    (rpc_code::INVALID_PARAMS, format!("invalid params: {e}"))
                }
                other => (rpc_code::INTERNAL_ERROR, other.to_string()),
            };
            RpcResponse::err(id, code, message)
        }
    }
}

fn handle_method(backend: &dyn ManagementBackend, method: &str, params: &Value) -> Result<Value> {
    // minecraft:<namespace>/<path> 拆分
    let rest = method
        .strip_prefix("minecraft:")
        .ok_or_else(|| crate::ManagementError::msg(format!("unknown method namespace: {method}")))?;
    let (namespace, path) = match rest.split_once('/') {
        Some((namespace, path)) => (namespace, format!("/{path}")),
        None => (rest, "/".to_string()),
    };

    match namespace {
        "allowlist" => handle_allowlist(backend, &path, params),
        "bans" => handle_bans(backend, &path, params),
        "ip_bans" => handle_ip_bans(backend, &path, params),
        "players" => handle_players(backend, &path, params),
        "operators" => handle_operators(backend, &path, params),
        "server" => handle_server(backend, &path, params),
        "serversettings" => handle_serversettings(backend, &path, params),
        "gamerules" => handle_gamerules(backend, &path, params),
        _ => Err(crate::ManagementError::msg(format!("unknown namespace: {namespace}"))),
    }
}

fn take<T: serde::de::DeserializeOwned>(params: &Value, key: &str) -> Result<T> {
    let value = params
        .get(key)
        .ok_or_else(|| crate::ManagementError::msg(format!("missing param: {key}")))?;
    serde_json::from_value(value.clone())
        .map_err(|e| crate::ManagementError::msg(format!("invalid param {key}: {e}")))
}

// ── allowlist ──
fn handle_allowlist(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/" => json!({ "allowlist": b.allowlist()? }),
        "/set" => json!({ "allowlist": b.allowlist_set(take::<Vec<AllowlistEntry>>(params, "players")?)? }),
        "/add" => json!({ "allowlist": b.allowlist_add(take::<Vec<AllowlistEntry>>(params, "add")?)? }),
        "/remove" => json!({ "allowlist": b.allowlist_remove(take::<Vec<AllowlistEntry>>(params, "remove")?)? }),
        "/clear" => json!({ "allowlist": b.allowlist_clear()? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: allowlist{path}"))),
    };
    Ok(result)
}

// ── bans ──
fn handle_bans(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/" => json!({ "banlist": b.bans()? }),
        "/set" => json!({ "banlist": b.bans_set(take::<Vec<UserBan>>(params, "bans")?)? }),
        "/add" => json!({ "banlist": b.bans_add(take::<Vec<UserBan>>(params, "add")?)? }),
        "/remove" => json!({ "banlist": b.bans_remove(take::<Vec<PlayerRef>>(params, "remove")?)? }),
        "/clear" => json!({ "banlist": b.bans_clear()? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: bans{path}"))),
    };
    Ok(result)
}

// ── ip_bans ──
fn handle_ip_bans(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/" => json!({ "banlist": b.ip_bans()? }),
        "/set" => json!({ "banlist": b.ip_bans_set(take::<Vec<IpBan>>(params, "bans")?)? }),
        "/add" => json!({ "banlist": b.ip_bans_add(take::<Vec<IpBan>>(params, "add")?)? }),
        "/remove" => json!({ "banlist": b.ip_bans_remove(take::<Vec<String>>(params, "ip")?)? }),
        "/clear" => json!({ "banlist": b.ip_bans_clear()? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: ip_bans{path}"))),
    };
    Ok(result)
}

// ── players ──
fn handle_players(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/" => json!({ "players": b.players()? }),
        "/kick" => json!({ "kicked": b.players_kick(take::<Vec<KickRequest>>(params, "kick")?)? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: players{path}"))),
    };
    Ok(result)
}

// ── operators ──
fn handle_operators(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/" => json!({ "operators": b.operators()? }),
        "/set" => json!({ "operators": b.operators_set(take::<Vec<OperatorEntry>>(params, "operators")?)? }),
        "/add" => json!({ "operators": b.operators_add(take::<Vec<OperatorEntry>>(params, "add")?)? }),
        "/remove" => json!({ "operators": b.operators_remove(take::<Vec<PlayerRef>>(params, "remove")?)? }),
        "/clear" => json!({ "operators": b.operators_clear()? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: operators{path}"))),
    };
    Ok(result)
}

// ── server ──
fn handle_server(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/status" => json!({ "status": b.server_status()? }),
        "/save" => json!({ "saving": b.server_save(take::<bool>(params, "flush")?)? }),
        "/stop" => json!({ "stopping": b.server_stop()? }),
        "/system_message" => json!({ "sent": b.server_system_message(take::<SystemMessage>(params, "message")?)? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: server{path}"))),
    };
    Ok(result)
}

// ── serversettings ──
fn handle_serversettings(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/autosave" => json!({ "enabled": b.autosave()? }),
        "/autosave/set" => json!({ "enabled": b.set_autosave(take::<bool>(params, "enable")?)? }),
        "/difficulty" => json!({ "difficulty": b.difficulty()? }),
        "/difficulty/set" => json!({ "difficulty": b.set_difficulty(take::<Difficulty>(params, "difficulty")?)? }),
        "/enforce_allowlist" => json!({ "enforced": b.enforce_allowlist()? }),
        "/enforce_allowlist/set" => json!({ "enforced": b.set_enforce_allowlist(take::<bool>(params, "enforce")?)? }),
        "/use_allowlist" => json!({ "used": b.use_allowlist()? }),
        "/use_allowlist/set" => json!({ "used": b.set_use_allowlist(take::<bool>(params, "use")?)? }),
        "/max_players" => json!({ "max": b.max_players()? }),
        "/max_players/set" => json!({ "max": b.set_max_players(take::<i32>(params, "max")?)? }),
        "/pause_when_empty_seconds" => json!({ "seconds": b.pause_when_empty_seconds()? }),
        "/pause_when_empty_seconds/set" => json!({ "seconds": b.set_pause_when_empty_seconds(take::<i32>(params, "seconds")?)? }),
        "/player_idle_timeout" => json!({ "seconds": b.player_idle_timeout()? }),
        "/player_idle_timeout/set" => json!({ "seconds": b.set_player_idle_timeout(take::<i32>(params, "seconds")?)? }),
        "/allow_flight" => json!({ "allowed": b.allow_flight()? }),
        "/allow_flight/set" => json!({ "allowed": b.set_allow_flight(take::<bool>(params, "allowed")?)? }),
        "/motd" => json!({ "message": b.motd()? }),
        "/motd/set" => json!({ "message": b.set_motd(take::<String>(params, "message")?)? }),
        "/spawn_protection_radius" => json!({ "radius": b.spawn_protection_radius()? }),
        "/spawn_protection_radius/set" => json!({ "radius": b.set_spawn_protection_radius(take::<i32>(params, "radius")?)? }),
        "/force_game_mode" => json!({ "forced": b.force_game_mode()? }),
        "/force_game_mode/set" => json!({ "forced": b.set_force_game_mode(take::<bool>(params, "force")?)? }),
        "/game_mode" => json!({ "mode": b.game_mode()? }),
        "/game_mode/set" => json!({ "mode": b.set_game_mode(take::<GameMode>(params, "mode")?)? }),
        "/view_distance" => json!({ "distance": b.view_distance()? }),
        "/view_distance/set" => json!({ "distance": b.set_view_distance(take::<i32>(params, "distance")?)? }),
        "/simulation_distance" => json!({ "distance": b.simulation_distance()? }),
        "/simulation_distance/set" => json!({ "distance": b.set_simulation_distance(take::<i32>(params, "distance")?)? }),
        "/accept_transfers" => json!({ "accepted": b.accept_transfers()? }),
        "/accept_transfers/set" => json!({ "accepted": b.set_accept_transfers(take::<bool>(params, "accept")?)? }),
        "/status_heartbeat_interval" => json!({ "seconds": b.status_heartbeat_interval()? }),
        "/status_heartbeat_interval/set" => json!({ "seconds": b.set_status_heartbeat_interval(take::<i64>(params, "seconds")?)? }),
        "/operator_user_permission_level" => json!({ "level": b.operator_user_permission_level()? }),
        "/operator_user_permission_level/set" => json!({ "level": b.set_operator_user_permission_level(take::<i32>(params, "level")?)? }),
        "/hide_online_players" => json!({ "hidden": b.hide_online_players()? }),
        "/hide_online_players/set" => json!({ "hidden": b.set_hide_online_players(take::<bool>(params, "hide")?)? }),
        "/status_replies" => json!({ "enabled": b.status_replies()? }),
        "/status_replies/set" => json!({ "enabled": b.set_status_replies(take::<bool>(params, "enable")?)? }),
        "/entity_broadcast_range" => json!({ "percentage_points": b.entity_broadcast_range()? }),
        "/entity_broadcast_range/set" => json!({ "percentage_points": b.set_entity_broadcast_range(take::<i32>(params, "percentage_points")?)? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: serversettings{path}"))),
    };
    Ok(result)
}

// ── gamerules ──
fn handle_gamerules(b: &dyn ManagementBackend, path: &str, params: &Value) -> Result<Value> {
    let result = match path {
        "/" => json!({ "gamerules": b.gamerules()? }),
        "/update" => json!({ "gamerule": b.gamerule_update(take::<UntypedGameRule>(params, "gamerule")?)? }),
        _ => return Err(crate::ManagementError::msg(format!("unknown path: gamerules{path}"))),
    };
    Ok(result)
}
