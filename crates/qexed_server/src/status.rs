//! 服务器列表 ping 响应（status 协议 JSON）。
//! 迁移自 v4 crates/qexed/src/status.rs；配置类型换成 v6 的 crate::config::ServerConfig。

use serde_json::json;

use crate::config::ServerConfig;

/// 构建 status ping 的 JSON 响应。
pub fn response(config: &ServerConfig) -> serde_json::Value {
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
            "text": config.motd.join("\n"),
        },
        "favicon": config.favicon,
        "enforcesSecureChat": effective_online_mode(config),
    })
}

/// 代理（Victory 转发）模式下以 proxy_online_mode 为准，否则用 online_mode。
fn effective_online_mode(config: &ServerConfig) -> bool {
    if config.proxy && config.proxy_protocol == crate::config::ForwardingMode::Victory {
        config.proxy_online_mode
    } else {
        config.online_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_reports_protocol_and_max_players() {
        let config = ServerConfig {
            max_player: -1,
            ..ServerConfig::default()
        };
        let value = response(&config);
        assert_eq!(value["version"]["protocol"], qexed_config::PROTOCOL_VERSION);
        assert_eq!(value["players"]["max"], 0);
        assert_eq!(value["enforcesSecureChat"], true);
    }
}
