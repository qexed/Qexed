//! 管理协议单元测试（JSON-RPC 编解码 / 认证 / 通知 / 密钥生成）。

use serde_json::{Value, json};

use crate::backend::{ManagementBackend, NoBackend};
use crate::config::ManagementConfig;
use crate::model::{PlayerRef, UntypedGameRule};
use crate::protocol::{RpcRequest, dispatch};
use crate::server::notify;

fn rpc(method: &str, params: Value) -> Value {
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!("t1")),
        method: method.to_string(),
        params: Some(params),
    };
    serde_json::to_value(dispatch(&NoBackend, &request)).unwrap()
}

#[test]
fn parse_error_on_garbage() {
    let request: Result<RpcRequest, _> = serde_json::from_str("not json");
    assert!(request.is_err());
}

#[test]
fn unknown_method_returns_error() {
    let response = rpc("minecraft:nonexistent/thing", json!({}));
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], json!(-32603));
}

#[test]
fn wrong_jsonrpc_version_rejected() {
    let request = RpcRequest {
        jsonrpc: "1.0".to_string(),
        id: Some(json!("t2")),
        method: "minecraft:players".to_string(),
        params: None,
    };
    let response = dispatch(&NoBackend, &request);
    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["error"]["code"], json!(-32600));
}

#[test]
fn notification_without_id_no_response_needed() {
    // dispatch 仍执行但调用方（handle_text）依据 id 判定不发响应——此处验证结构可解码。
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: "minecraft:notification/players/joined".to_string(),
        params: Some(json!({"player": {"name": "Steve"}})),
    };
    // 无 id 的通知进入 dispatch 也应正常（返回给 Null id）
    let response = dispatch(&NoBackend, &request);
    assert!(serde_json::to_value(&response).is_ok());
}

#[test]
fn secret_generation_is_40_alphanumeric() {
    let secret = ManagementConfig::generate_secret();
    assert_eq!(secret.len(), 40);
    assert!(secret.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn ensure_secret_fills_empty_only() {
    let mut config = ManagementConfig::default();
    assert!(config.secret.is_empty());
    config.ensure_secret();
    assert_eq!(config.secret.len(), 40);
    let existing = config.secret.clone();
    config.ensure_secret();
    assert_eq!(config.secret, existing, "已有密钥不被覆盖");
}

#[test]
fn notification_encoding_shape() {
    let notification = notify::player_joined(&PlayerRef::by_name("Alex"));
    let encoded: Value = serde_json::from_str(&notification.encode()).unwrap();
    assert_eq!(encoded["jsonrpc"], json!("2.0"));
    assert_eq!(encoded["method"], json!("minecraft:notification/players/joined"));
    assert_eq!(encoded["params"]["player"]["name"], json!("Alex"));
}

#[test]
fn gamerule_notification_roundtrip() {
    let notification = notify::gamerule_updated(&crate::model::TypedGameRule {
        key: "minecraft:doDaylightCycle".to_string(),
        rule_type: "boolean".to_string(),
        value: json!(true),
    });
    let encoded: Value = serde_json::from_str(&notification.encode()).unwrap();
    assert_eq!(encoded["params"]["gamerule"]["key"], json!("minecraft:doDaylightCycle"));
    assert_eq!(encoded["params"]["gamerule"]["value"], json!(true));
}

#[test]
fn world_upgrade_progress_clamped() {
    let notification = notify::world_upgrade_progress(1.5);
    let encoded: Value = serde_json::from_str(&notification.encode()).unwrap();
    assert_eq!(encoded["params"]["progress"], json!(1.0));
}

#[test]
fn untyped_gamerule_params_decode() {
    // /update 请求的参数形态
    let value = json!({
        "gamerule": {
            "key": "minecraft:maxEntityCramming",
            "value": "24"
        }
    });
    let rule: UntypedGameRule = serde_json::from_value(value["gamerule"].clone()).unwrap();
    assert_eq!(rule.key, "minecraft:maxEntityCramming");
    assert_eq!(rule.value, json!("24"));
}

#[test]
fn player_ref_serializes_skip_none() {
    let player = PlayerRef::by_name("Notch");
    let value = serde_json::to_value(&player).unwrap();
    assert!(value.get("id").is_none());
    assert_eq!(value["name"], json!("Notch"));
}
