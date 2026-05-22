use serde_json::json;

pub fn response(config: &qexed_config::app::qexed::Qexed) -> serde_json::Value {
    let max_players = if config.server.max_player < 0 {
        0
    } else {
        config.server.max_player
    };

    let players = if config.server.display_players {
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
            "text": config.server.motd.join("\n"),
        },
        "favicon": config.server.favicon,
        "enforcesSecureChat": config.server.online_mode,
    })
}
