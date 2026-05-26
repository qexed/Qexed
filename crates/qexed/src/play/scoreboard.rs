use bytes::Bytes;
use qexed_config::app::qexed::server::Scoreboard;
use qexed_protocol::to_client::play::{
    set_display_objective::SetDisplayObjective, set_objective::SetObjective, set_score::SetScore,
};

use super::util::text_component;

const MAX_OBJECTIVE_NAME_LEN: usize = 16;
const MAX_SIDEBAR_LINES: usize = 15;

pub(super) fn sidebar_packets(config: &Scoreboard) -> anyhow::Result<Vec<Bytes>> {
    if !config.enable {
        return Ok(Vec::new());
    }

    let objective_name = objective_name(&config.objective);
    let mut packets = vec![
        packet_bytes(SetObjective::create(
            objective_name.clone(),
            text_component(&config.title),
        ))?,
        packet_bytes(SetDisplayObjective::sidebar(objective_name.clone()))?,
    ];

    let lines = config.lines.iter().take(MAX_SIDEBAR_LINES);
    for (index, line) in lines.enumerate() {
        packets.push(packet_bytes(SetScore::new(
            line_owner(index),
            objective_name.clone(),
            (MAX_SIDEBAR_LINES - index) as i32,
            Some(text_component(line)),
        ))?);
    }

    Ok(packets)
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
        let packets = super::sidebar_packets(&Scoreboard::default()).unwrap();
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

        let packets = super::sidebar_packets(&config).unwrap();
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
}
