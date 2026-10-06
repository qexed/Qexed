//! 状态 ping 响应构造（v4 status.rs 迁移）。
//!
//! v4 的 qexed_config::app::qexed::Qexed 配置在 v6 不存在，
//! 改用本 crate 的 config::ConnectionConfig。

use serde_json::json;

use crate::config::ConnectionConfig;

/// 服务器列表 ping 的 JSON 响应体。
pub fn response(config: &ConnectionConfig) -> serde_json::Value {
    let max_players = config.max_player.max(0);

    let players = if config.display_players {
        json!({
            "max": max_players,
            "online": 0,
            "sample": [],
        })
    } else {
        json!({
            "max": max_players,
            "online": 0,
        })
    };

    json!({
        "version": {
            "name": qexed_config::MC_VERSION,
            "protocol": qexed_config::PROTOCOL_VERSION,
        },
        "players": players,
        "description": {
            "text": config.motd,
        },
        "favicon": config.favicon,
        "enforcesSecureChat": effective_online_mode(config),
    })
}

fn effective_online_mode(config: &ConnectionConfig) -> bool {
    if config.proxy
        && config.proxy_protocol == crate::config::ForwardingMode::Victory
    {
        config.proxy_online_mode
    } else {
        config.online_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_status_response() {
        let config = ConnectionConfig::default();
        let value = response(&config);
        assert_eq!(
            value["version"]["protocol"],
            serde_json::json!(qexed_config::PROTOCOL_VERSION)
        );
        assert_eq!(value["players"]["max"], serde_json::json!(0));
    }
}
