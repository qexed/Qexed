use bytes::Bytes;
use qexed_protocol::to_client::play::{
    set_display_objective::SetDisplayObjective, set_objective::SetObjective, set_score::SetScore,
};

use crate::config::Scoreboard;
use crate::context::PlaceholderContext;

use super::util::text_component;

const MAX_OBJECTIVE_NAME_LEN: usize = 16;
const MAX_SIDEBAR_LINES: usize = 15;

pub(super) fn lobby_sidebar_packets(
    config: &Scoreboard,
    lobby: &super::lobby::LobbyRuntime,
    status: &super::lobby::LobbyStatusSnapshot,
    placeholders_enabled: bool,
    plugins: &qexed_plugins::PluginManager,
    player: &qexed_player::OnlinePlayer,
    online_players: usize,
    max_players: i32,
) -> crate::error::Result<Vec<Bytes>> {
    let lines = render_lobby_lines(
        config,
        lobby,
        status,
        placeholders_enabled,
        plugins,
        player,
        online_players,
        max_players,
    );
    sidebar_packets_with_lines(config, &lines, Some(&config.title))
}

pub(super) fn refresh_lobby_sidebar_packets(
    config: &Scoreboard,
    lobby: &super::lobby::LobbyRuntime,
    status: &super::lobby::LobbyStatusSnapshot,
    placeholders_enabled: bool,
    plugins: &qexed_plugins::PluginManager,
    player: &qexed_player::OnlinePlayer,
    online_players: usize,
    max_players: i32,
) -> crate::error::Result<Vec<Bytes>> {
    if !config.enable {
        return Ok(Vec::new());
    }

    let objective_name = objective_name(&config.objective);
    let mut packets = Vec::new();
    let lines = render_lobby_lines(
        config,
        lobby,
        status,
        placeholders_enabled,
        plugins,
        player,
        online_players,
        max_players,
    );
    for (index, line) in lines.iter().take(MAX_SIDEBAR_LINES).enumerate() {
        packets.push(packet_bytes(SetScore::new(
            line_owner(index),
            objective_name.clone(),
            (MAX_SIDEBAR_LINES - index) as i32,
            Some(text_component(line)),
        ))?);
    }

    Ok(packets)
}

pub(super) fn custom_sidebar_packets(
    objective: &str,
    title: &str,
    lines: &[String],
) -> crate::error::Result<Vec<Bytes>> {
    let config = Scoreboard {
        enable: true,
        objective: objective.to_string(),
        title: title.to_string(),
        lines: lines.to_vec(),
    };
    sidebar_packets_with_lines(&config, lines, Some(title))
}

pub(super) fn clear_sidebar_packet(objective: &str) -> crate::error::Result<Vec<Bytes>> {
    Ok(vec![
        packet_bytes(SetDisplayObjective::clear_sidebar())?,
        packet_bytes(set_objective_remove(objective_name(objective)))?,
    ])
}

fn sidebar_packets_with_lines(
    config: &Scoreboard,
    lines: &[String],
    title: Option<&str>,
) -> crate::error::Result<Vec<Bytes>> {
    if !config.enable {
        return Ok(Vec::new());
    }

    let objective_name = objective_name(&config.objective);
    let mut packets = vec![
        packet_bytes(set_objective_create(
            objective_name.clone(),
            text_component(title.unwrap_or(&config.title)),
        ))?,
        packet_bytes(SetDisplayObjective::sidebar(objective_name.clone()))?,
    ];

    for (index, line) in lines.iter().take(MAX_SIDEBAR_LINES).enumerate() {
        packets.push(packet_bytes(SetScore::new(
            line_owner(index),
            objective_name.clone(),
            (MAX_SIDEBAR_LINES - index) as i32,
            Some(text_component(line)),
        ))?);
    }

    Ok(packets)
}

fn render_lobby_lines(
    config: &Scoreboard,
    lobby: &super::lobby::LobbyRuntime,
    status: &super::lobby::LobbyStatusSnapshot,
    placeholders_enabled: bool,
    plugins: &qexed_plugins::PluginManager,
    player: &qexed_player::OnlinePlayer,
    online_players: usize,
    max_players: i32,
) -> Vec<String> {
    let labels = lobby.server_labels(status);
    let context = PlaceholderContext {
        online_players: online_players as i32,
        max_players,
        lobby_online_servers: lobby.online_server_count(status),
        lobby_total_servers: lobby.total_server_count(status),
        lobby_servers: if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join(", ")
        },
    };
    config
        .lines
        .iter()
        .map(|line| {
            render_placeholders(
                placeholders_enabled,
                plugins,
                Some(player),
                line,
                &context,
            )
        })
        .collect()
}

/**
 * 本地占位符渲染（v4 qexed_server::placeholders::format_placeholders 的 play 子集）。
 *
 * 迁移规则禁止 qexed_play 依赖 qexed_server，因此侧边栏只用原生占位符
 * （%online_players% 等全局/大厅 token + {servers} 大括号别名）；插件占位符
 * （PlaceholderProvider 查询）归 server 域，TODO(assembly)：装配层若需完整
 * 插件占位符，经 context::PlaceholderRenderer 注入后替换本实现。
 */
fn render_placeholders(
    enabled: bool,
    plugins: &qexed_plugins::PluginManager,
    player: Option<&qexed_player::OnlinePlayer>,
    text: &str,
    context: &PlaceholderContext,
) -> String {
    let _ = plugins;
    if !enabled {
        return text.to_string();
    }
    let mut rendered = text.to_string();
    rendered = rendered
        .replace("%online_players%", &context.online_players.to_string())
        .replace("%max_players%", &context.max_players.to_string())
        .replace(
            "%lobby_online_servers%",
            &context.lobby_online_servers.to_string(),
        )
        .replace(
            "%lobby_total_servers%",
            &context.lobby_total_servers.to_string(),
        )
        .replace("%lobby_servers%", &context.lobby_servers)
        .replace("{online_servers}", &context.lobby_online_servers.to_string())
        .replace("{total_servers}", &context.lobby_total_servers.to_string())
        .replace("{servers}", &context.lobby_servers);
    if let Some(player) = player {
        rendered = rendered
            .replace("%player_name%", &player.profile.username)
            .replace("%player_uuid%", &player.profile.uuid.to_string())
            .replace("%player_language%", &player.language)
            .replace("%player_dimension%", &player.dimension);
    }
    rendered
}

/// v4 SetObjective::create 的本地等价（v6 协议 crate 无该构造器）。
pub(super) fn set_objective_create(
    objective_name: impl Into<String>,
    display_name: qexed_protocol::types::TextComponent,
) -> SetObjective {
    SetObjective {
        objective_name: objective_name.into(),
        method: METHOD_OBJECTIVE_ADD,
        display_name,
        render_type: qexed_packet::net_types::VarInt(RENDER_TYPE_INTEGER),
        number_format: None,
    }
}

/// v4 SetObjective::remove 的本地等价。
pub(super) fn set_objective_remove(objective_name: impl Into<String>) -> SetObjective {
    SetObjective {
        objective_name: objective_name.into(),
        method: METHOD_OBJECTIVE_REMOVE,
        display_name: qexed_protocol::types::TextComponent::default(),
        render_type: qexed_packet::net_types::VarInt(RENDER_TYPE_INTEGER),
        number_format: None,
    }
}

/// SetObjective method 字段取值（v4 set_objective::METHOD_*；26.3 语义不变）。
pub(super) const METHOD_OBJECTIVE_ADD: i8 = 0;
const METHOD_OBJECTIVE_REMOVE: i8 = 1;
const RENDER_TYPE_INTEGER: i32 = 0;

fn objective_name(configured: &str) -> String {
    let mut name: String = configured
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .take(MAX_OBJECTIVE_NAME_LEN)
        .collect();
    if name.is_empty() {
        name = "qexed".to_string();
    }
    name
}

fn line_owner(index: usize) -> String {
    format!("qexed_line_{index}")
}

fn packet_bytes<T: qexed_packet::Packet>(packet: T) -> crate::error::Result<Bytes> {
    qexed_player::packet_bytes(packet).map_err(crate::error::PlayError::from)
}

#[cfg(test)]
mod tests {
    use crate::config::Scoreboard;
    use qexed_packet::{Packet, PacketCodec, PacketReader, net_types::VarInt};
    use qexed_protocol::to_client::play::{
        set_display_objective::SetDisplayObjective, set_objective::SetObjective,
        set_score::SetScore,
    };

    #[test]
    fn disabled_scoreboard_sends_no_packets() {
        let config = Scoreboard::default();
        let packets = super::sidebar_packets_with_lines(&config, &config.lines, None).unwrap();
        assert!(packets.is_empty());
    }

    #[test]
    fn sidebar_packets_create_objective_display_and_scores() {
        let config = Scoreboard {
            enable: true,
            objective: "qexed.example/objective-too-long".to_string(),
            title: "Qexed Test".to_string(),
            lines: vec!["first".to_string(), "second".to_string()],
        };

        let packets = super::sidebar_packets_with_lines(&config, &config.lines, None).unwrap();
        assert_eq!(packets.len(), 4);
        assert_packet_id::<SetObjective>(&packets[0]);
        assert_packet_id::<SetDisplayObjective>(&packets[1]);
        assert_packet_id::<SetScore>(&packets[2]);
        assert_packet_id::<SetScore>(&packets[3]);

        let objective = decode_packet::<SetObjective>(&packets[0]);
        assert_eq!(objective.objective_name, "qexed.exampleobj");
        assert_eq!(objective.method, super::METHOD_OBJECTIVE_ADD);

        let first_score = decode_packet::<SetScore>(&packets[2]);
        assert_eq!(first_score.owner, "qexed_line_0");
        assert_eq!(first_score.objective_name, objective.objective_name);
        assert_eq!(first_score.score.0, 15);
        assert!(first_score.display.is_some());
        assert_eq!(
            first_score.number_format,
            Some(qexed_protocol::types::NumberFormat::Blank)
        );
    }

    #[test]
    fn lobby_sidebar_packets_render_status_placeholders() {
        let config = Scoreboard {
            enable: true,
            objective: "qexed".to_string(),
            title: "Qexed".to_string(),
            lines: vec![
                "Backends: {online_servers}/{total_servers}".to_string(),
                "{servers}".to_string(),
            ],
        };
        let lobby =
            super::super::lobby::LobbyRuntime::new(&crate::config::Lobby {
                enable: true,
                menu_items: vec![
                    transfer_item(10, "survival", "Survival"),
                    transfer_item(11, "minigame", "Minigame"),
                ],
                ..Default::default()
            });
        let mut servers = std::collections::HashMap::new();
        servers.insert(
            "survival".to_string(),
            super::super::lobby::LobbyServerStatus::Online,
        );
        servers.insert(
            "minigame".to_string(),
            super::super::lobby::LobbyServerStatus::Offline,
        );
        let status = super::super::lobby::LobbyStatusSnapshot::from_servers_for_tests(servers);

        let plugins = qexed_plugins::PluginManager::empty_for_tests();
        let player = test_player();
        let packets =
            super::lobby_sidebar_packets(&config, &lobby, &status, true, &plugins, &player, 1, 20)
                .unwrap();
        assert_eq!(packets.len(), 4);

        let first_score = decode_packet::<SetScore>(&packets[2]);
        assert_text_component(first_score.display.as_ref().unwrap(), "Backends: 1/2");

        let refreshed = super::refresh_lobby_sidebar_packets(
            &config, &lobby, &status, true, &plugins, &player, 1, 20,
        )
        .unwrap();
        assert_eq!(refreshed.len(), 2);
        let refreshed_first = decode_packet::<SetScore>(&refreshed[0]);
        assert_text_component(refreshed_first.display.as_ref().unwrap(), "Backends: 1/2");
    }

    fn assert_packet_id<T: Packet>(bytes: &[u8]) {
        let mut bytes = bytes::Bytes::copy_from_slice(bytes);
        let mut reader = PacketReader::new(&mut bytes);
        let mut id = VarInt::default();
        id.deserialize(&mut reader).unwrap();
        assert_eq!(id.0, T::ID);
    }

    fn decode_packet<T: Packet>(bytes: &[u8]) -> T {
        let mut bytes = bytes::Bytes::copy_from_slice(bytes);
        let mut reader = PacketReader::new(&mut bytes);
        let mut id = VarInt::default();
        id.deserialize(&mut reader).unwrap();
        assert_eq!(id.0, T::ID);

        let mut packet = T::default();
        packet.deserialize(&mut reader).unwrap();
        packet
    }

    fn assert_text_component(component: &qexed_protocol::types::TextComponent, expected: &str) {
        let qexed_nbt::Tag::Compound(map) = component else {
            panic!("expected text component compound");
        };
        assert_eq!(
            map.get("text"),
            Some(&qexed_nbt::Tag::String(std::sync::Arc::from(expected)))
        );
    }

    fn test_player() -> qexed_player::OnlinePlayer {
        qexed_player::OnlinePlayer {
            profile: qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::nil(),
                username: "Tester".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            game_mode: 0,
            position: qexed_protocol::types::EntityPosition::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "en_us".to_string(),
            displayed_skin_parts: qexed_player::DEFAULT_DISPLAYED_SKIN_PARTS,
        }
    }

    fn transfer_item(
        slot: u8,
        target: &str,
        name: &str,
    ) -> crate::config::LobbyMenuItem {
        crate::config::LobbyMenuItem {
            slot,
            name: name.to_string(),
            action: crate::config::LobbyAction {
                kind: crate::config::LobbyActionKind::Transfer,
                target: target.to_string(),
                message: String::new(),
            },
            ..Default::default()
        }
    }
}
