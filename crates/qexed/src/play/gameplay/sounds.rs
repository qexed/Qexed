use anyhow::Result;
use qexed_protocol::to_client::play::{add_entity::EntityPosition, sound::Sound};

pub(in crate::play) async fn play_at<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    players: &crate::players::PlayerManager,
    plugins: &crate::plugins::PluginManager,
    player: Option<&crate::players::OnlinePlayer>,
    dimension: &str,
    position: EntityPosition,
    sound: &str,
    source: &str,
    enabled: bool,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if !enabled {
        return Ok(());
    }
    let payload = crate::plugins::SoundPayload {
        player: player.map(qexed_plugin_api::player_payload_owned),
        sound: sound.to_string(),
        source: source.to_string(),
        dimension: dimension.to_string(),
        x: position.x,
        y: position.y,
        z: position.z,
        volume: 1.0,
        pitch: 1.0,
    };
    let response = plugins.apply_sound(payload);
    if response.cancel {
        return Ok(());
    }
    let sound_name = response.sound.as_deref().unwrap_or(sound);
    let Some(sound_id) = sound_event_id(sound_name) else {
        log::debug!("skip unknown sound event: {sound_name}");
        return Ok(());
    };
    let packet = Sound {
        volume: response.volume.unwrap_or(1.0).clamp(0.0, 16.0),
        pitch: response.pitch.unwrap_or(1.0).clamp(0.01, 4.0),
        ..Sound::at_block(
            sound_id,
            sound_source_id(source),
            position.x,
            position.y,
            position.z,
        )
    };
    let bytes = crate::players::packet_bytes(packet.clone())?;
    sink.send(packet).await?;
    let actor = player
        .map(|player| player.profile.uuid)
        .unwrap_or(uuid::Uuid::nil());
    for viewer in players.list_except(actor) {
        if viewer.dimension == dimension {
            players.send_packets_to(viewer.profile.uuid, vec![bytes.clone()]);
        }
    }
    Ok(())
}

pub(super) fn sound_event_id(name: &str) -> Option<i32> {
    static IDS: std::sync::OnceLock<std::collections::HashMap<String, i32>> =
        std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        crate::registry_sync::load_registry_id_map("minecraft:sound_event").unwrap_or_default()
    })
    .get(name)
    .copied()
    .or_else(|| fallback_sound_event_id(name))
}

fn fallback_sound_event_id(name: &str) -> Option<i32> {
    match name {
        "minecraft:block.stone.break" => Some(1280),
        "minecraft:block.stone.place" => Some(1284),
        "minecraft:entity.generic.eat" => Some(558),
        "minecraft:entity.player.attack.strong" => Some(1440),
        "minecraft:entity.player.hurt" => Some(1465),
        "minecraft:entity.player.levelup" => Some(1460),
        "minecraft:ui.toast.challenge_complete" => Some(1600),
        _ => None,
    }
}

fn sound_source_id(source: &str) -> i32 {
    match source {
        "master" => 0,
        "music" => 1,
        "record" | "records" => 2,
        "weather" => 3,
        "block" | "blocks" => 4,
        "hostile" => 5,
        "neutral" => 6,
        "player" | "players" => 7,
        "ambient" => 8,
        "voice" => 9,
        _ => 0,
    }
}
