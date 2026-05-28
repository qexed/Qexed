use bytes::Bytes;
use qexed_config::app::qexed::server::Scoreboard;
use qexed_protocol::to_client::play::{
    set_display_objective::SetDisplayObjective, set_objective::SetObjective, set_score::SetScore,
};

use super::util::text_component;

const MAX_OBJECTIVE_NAME_LEN: usize = 16;
const MAX_SIDEBAR_LINES: usize = 15;

pub(super) fn lobby_sidebar_packets(
    config: &Scoreboard,
    lobby: &super::lobby::LobbyRuntime,
    status: &super::lobby::LobbyStatusSnapshot,
    placeholders_enabled: bool,
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    online_players: usize,
    max_players: i32,
) -> anyhow::Result<Vec<Bytes>> {
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
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    online_players: usize,
    max_players: i32,
) -> anyhow::Result<Vec<Bytes>> {
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
) -> anyhow::Result<Vec<Bytes>> {
    let config = Scoreboard {
        enable: true,
        objective: objective.to_string(),
        title: title.to_string(),
        lines: lines.to_vec(),
    };
    sidebar_packets_with_lines(&config, lines, Some(title))
}

pub(super) fn clear_sidebar_packet(objective: &str) -> anyhow::Result<Vec<Bytes>> {
    Ok(vec![
        packet_bytes(SetDisplayObjective::clear_sidebar())?,
        packet_bytes(SetObjective::remove(objective_name(objective)))?,
    ])
}

fn sidebar_packets_with_lines(
    config: &Scoreboard,
    lines: &[String],
    title: Option<&str>,
) -> anyhow::Result<Vec<Bytes>> {
    if !config.enable {
        return Ok(Vec::new());
    }

    let objective_name = objective_name(&config.objective);
    let mut packets = vec![
        packet_bytes(SetObjective::create(
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
    plugins: &crate::plugins::PluginManager,
    player: &crate::players::OnlinePlayer,
    online_players: usize,
    max_players: i32,
) -> Vec<String> {
    let labels = lobby.server_labels(status);
    let context = crate::placeholders::PlaceholderContext {
        online_players,
        max_players,
        lobby_online_servers: labels.len(),
        lobby_total_servers: lobby.server_count(),
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
            crate::placeholders::format_placeholders(
                placeholders_enabled,
                plugins,
                Some(player),
                line,
                &context,
            )
        })
        .collect()
}

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

fn packet_bytes<T: qexed_packet::Packet>(packet: T) -> anyhow::Result<Bytes> {
    crate::players::packet_bytes(packet)
}

#[cfg(test)]
mod tests {
    use qexed_config::app::qexed::server::Scoreboard;
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
        assert_eq!(
            objective.method,
            qexed_protocol::to_client::play::set_objective::METHOD_ADD
        );

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
            super::super::lobby::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
                enable: true,
                servers: vec![qexed_config::app::qexed::server::LobbyServer {
                    id: "survival".to_string(),
                    name: "Survival".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            });
        let mut servers = std::collections::HashMap::new();
        servers.insert(
            "survival".to_string(),
            super::super::lobby::LobbyServerStatus::Online,
        );
        let status = super::super::lobby::LobbyStatusSnapshot::from_servers_for_tests(servers);

        let plugins = crate::plugins::PluginManager::empty_for_tests();
        let player = test_player();
        let packets =
            super::lobby_sidebar_packets(&config, &lobby, &status, true, &plugins, &player, 1, 20)
                .unwrap();
        assert_eq!(packets.len(), 4);

        let first_score = decode_packet::<SetScore>(&packets[2]);
        assert_text_component(first_score.display.as_ref().unwrap(), "Backends: 1/1");

        let refreshed = super::refresh_lobby_sidebar_packets(
            &config, &lobby, &status, true, &plugins, &player, 1, 20,
        )
        .unwrap();
        assert_eq!(refreshed.len(), 2);
        let refreshed_first = decode_packet::<SetScore>(&refreshed[0]);
        assert_text_component(refreshed_first.display.as_ref().unwrap(), "Backends: 1/1");
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

    fn test_player() -> crate::players::OnlinePlayer {
        crate::players::OnlinePlayer {
            profile: qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::nil(),
                username: "Tester".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            position: qexed_protocol::to_client::play::add_entity::EntityPosition::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "en_us".to_string(),
        }
    }
}
