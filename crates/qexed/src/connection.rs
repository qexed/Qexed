use bytes::{BufMut as _, BytesMut};
use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{GameProfile, JsonValue, RestBuffer, VarInt},
};
use qexed_protocol::{
    to_client,
    to_server::{
        configuration::{
            finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
            select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
            settings::Settings as ServerboundSettings,
        },
        handshaking::set_protocol::SetProtocol,
        login::{login_acknowledged::LoginAcknowledged, login_start::LoginStart},
        status::{ping::Ping as ServerboundPing, ping_start::PingStart},
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};

const SERVER_BRAND: &str = "qexed";
const MAX_PACKET_SIZE: usize = 2 * 1024 * 1024;
static SERVER_SESSION_ID: std::sync::OnceLock<uuid::Uuid> = std::sync::OnceLock::new();

pub async fn handle(stream: TcpStream, config: qexed_config::app::qexed::Qexed) {
    let peer = stream.peer_addr().ok();
    if let Err(err) = handle_inner(stream, &config).await {
        if is_expected_disconnect(&err) {
            return;
        }

        match peer {
            Some(peer) => tklog::warn!(format!("connection closed: peer={peer}, error={err:#}")),
            None => tklog::warn!(format!("connection closed: error={err:#}")),
        }
    }
}

fn is_expected_disconnect(err: &anyhow::Error) -> bool {
    err.downcast_ref::<std::io::Error>().is_some_and(|err| {
        matches!(
            err.kind(),
            std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::ConnectionReset
        )
    })
}

async fn handle_inner(
    mut stream: TcpStream,
    config: &qexed_config::app::qexed::Qexed,
) -> anyhow::Result<()> {
    let handshake = read_expected_packet::<SetProtocol>(&mut stream).await?;
    match handshake.next_state.0 {
        1 => handle_status(&mut stream, config).await,
        2 | 3 => handle_login(&mut stream, config, handshake).await,
        state => anyhow::bail!("unsupported handshake target state: {state}"),
    }
}

async fn handle_status(
    stream: &mut TcpStream,
    config: &qexed_config::app::qexed::Qexed,
) -> anyhow::Result<()> {
    read_expected_packet::<PingStart>(stream).await?;

    send_packet(
        stream,
        &to_client::status::server_info::ServerInfo {
            response: JsonValue(serde_json::json!({
                "version": {
                    "name": qexed_config::MC_VERSION,
                    "protocol": qexed_config::PROTOCOL_VERSION,
                },
                "players": {
                    "max": config.server.max_players,
                    "online": 0,
                },
                "description": {
                    "text": config.server.motd,
                },
            })),
        },
    )
    .await?;

    if let Ok(ping) = read_expected_packet::<ServerboundPing>(stream).await {
        send_packet(stream, &to_client::status::ping::Ping { time: ping.time }).await?;
    }

    Ok(())
}

async fn handle_login(
    stream: &mut TcpStream,
    config: &qexed_config::app::qexed::Qexed,
    handshake: SetProtocol,
) -> anyhow::Result<()> {
    if handshake.protocol_version.0 != qexed_config::PROTOCOL_VERSION {
        send_login_disconnect(
            stream,
            format!(
                "Unsupported protocol {}. This server expects {} ({})",
                handshake.protocol_version.0,
                qexed_config::PROTOCOL_VERSION,
                qexed_config::MC_VERSION
            ),
        )
        .await?;
        return Ok(());
    }

    let login_start = read_expected_packet::<LoginStart>(stream).await?;
    let profile = offline_profile(login_start);

    send_packet(
        stream,
        &to_client::login::success::Success {
            game_profile: profile.clone(),
            session_id: server_session_id(),
        },
    )
    .await?;

    read_expected_packet::<LoginAcknowledged>(stream).await?;
    handle_configuration(stream).await?;
    enter_play(stream, config, &profile).await
}

async fn handle_configuration(stream: &mut TcpStream) -> anyhow::Result<()> {
    send_packet(
        stream,
        &to_client::configuration::custom_payload::CustomPayload {
            channel: "minecraft:brand".to_string(),
            data: RestBuffer(string_payload(SERVER_BRAND)?),
        },
    )
    .await?;

    send_packet(
        stream,
        &to_client::configuration::feature_flags::FeatureFlags {
            features: vec![qexed_registry::VANILLA_FEATURE.to_string()],
        },
    )
    .await?;

    send_packet(
        stream,
        &to_client::configuration::select_known_packs::SelectKnownPacks {
            known_packs: qexed_registry::known_packs(),
        },
    )
    .await?;

    let selected_packs = wait_for_known_packs(stream).await?;
    let include_contents = !qexed_registry::accepts_vanilla_core_pack(&selected_packs.entries);
    for packet in qexed_registry::load_registry_packets(include_contents)? {
        send_packet(stream, &packet).await?;
    }

    let tags = qexed_registry::load_tag_packet()?;
    send_packet(stream, &tags).await?;

    send_packet(
        stream,
        &to_client::configuration::finish_configuration::FinishConfiguration {},
    )
    .await?;
    wait_for_finish_configuration(stream).await
}

async fn wait_for_known_packs(
    stream: &mut TcpStream,
) -> anyhow::Result<ServerboundSelectKnownPacks> {
    loop {
        let mut payload = read_frame(stream).await?;
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundSelectKnownPacks::ID {
            return decode_payload::<ServerboundSelectKnownPacks>(&mut payload);
        }
        if packet_id == ServerboundSettings::ID {
            let _settings = decode_payload::<ServerboundSettings>(&mut payload)?;
            continue;
        }
        tklog::debug!(format!(
            "skip configuration packet while waiting known packs: {packet_id}"
        ));
    }
}

async fn wait_for_finish_configuration(stream: &mut TcpStream) -> anyhow::Result<()> {
    loop {
        let mut payload = read_frame(stream).await?;
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundFinishConfiguration::ID {
            let _packet = decode_payload::<ServerboundFinishConfiguration>(&mut payload)?;
            return Ok(());
        }
        if packet_id == ServerboundSettings::ID {
            let _settings = decode_payload::<ServerboundSettings>(&mut payload)?;
            continue;
        }
        tklog::debug!(format!(
            "skip configuration packet while waiting finish: {packet_id}"
        ));
    }
}

async fn enter_play(
    stream: &mut TcpStream,
    config: &qexed_config::app::qexed::Qexed,
    profile: &GameProfile,
) -> anyhow::Result<()> {
    let view_distance = config.server.view_distance.max(1);
    let simulation_distance = config.server.simulation_distance.max(1);
    let dimension_type = qexed_registry::dimension_type_holder_id("minecraft:overworld")?;

    send_packet(
        stream,
        &to_client::play::login::Login {
            entity_id: 1,
            is_hardcore: false,
            dimension_names: vec!["minecraft:overworld".to_string()],
            max_player: VarInt(config.server.max_players.max(0)),
            view_distance: VarInt(view_distance),
            simulation_distance: VarInt(simulation_distance),
            reduced_debug_info: false,
            enable_respawn_screen: true,
            do_limited_crafting: false,
            dimension_type: VarInt(dimension_type),
            dimension_name: "minecraft:overworld".to_string(),
            hashed_seed: 0,
            game_mode: 1,
            previous_game_mode: -1,
            is_debug: false,
            is_flat: true,
            has_death_location: false,
            death_dimension_name: None,
            death_position: None,
            portal_cooldown: VarInt(0),
            sea_level: VarInt(63),
            online_mode: false,
            enforces_secure_chat: false,
        },
    )
    .await?;

    send_packet(
        stream,
        &to_client::play::position::Position {
            teleport_id: VarInt(1),
            x: 0.5,
            y: 64.0,
            z: 0.5,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            flags: 0,
        },
    )
    .await?;

    send_packet(
        stream,
        &to_client::play::change_difficulty::ChangeDifficulty {
            difficulty: 2,
            locked: false,
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::set_held_slot::SetHeldSlot { slot: VarInt(0) },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::server_data::ServerData {
            motd: text_component(config.server.motd.clone()),
            icon_bytes: None,
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::player_info_update::PlayerInfoUpdate {
            actions: to_client::play::player_info_update::PlayerInfoActions::player_initializing(),
            entries: vec![
                to_client::play::player_info_update::PlayerInfoEntry::from_profile(profile, 1),
            ],
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::player_abilities::PlayerAbilities {
            flags: to_client::play::player_abilities::PlayerAbilities::INVULNERABLE
                | to_client::play::player_abilities::PlayerAbilities::CAN_FLY
                | to_client::play::player_abilities::PlayerAbilities::INSTABUILD,
            flying_speed: 0.05,
            walking_speed: 0.1,
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::set_health::SetHealth {
            health: 20.0,
            food: VarInt(20),
            saturation: 5.0,
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::set_experience::SetExperience {
            experience_progress: 0.0,
            experience_level: VarInt(0),
            total_experience: VarInt(0),
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::initialize_border::InitializeBorder::default(),
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::set_time::SetTime {
            game_time: 0,
            clock_updates: Vec::new(),
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::set_default_spawn_position::SetDefaultSpawnPosition {
            dimension: "minecraft:overworld".to_string(),
            position: qexed_packet::net_types::Position { x: 0, y: 64, z: 0 },
            yaw: 0.0,
            pitch: 0.0,
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::game_state_change::GameStateChange {
            reason: 13,
            game_mode: 0.0,
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::set_simulation_distance::SetSimulationDistance {
            simulation_distance: VarInt(simulation_distance),
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::update_view_distance::UpdateViewDistance {
            view_distance: VarInt(view_distance),
        },
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::update_view_position::UpdateViewPosition {
            chunk_x: VarInt(0),
            chunk_z: VarInt(0),
        },
    )
    .await?;
    send_initial_chunks(stream, view_distance).await?;
    send_packet(
        stream,
        &to_client::play::ticking_state::TickingState::default(),
    )
    .await?;
    send_packet(
        stream,
        &to_client::play::system_chat::SystemChat {
            content: text_component(format!("{} joined qexed-v5", profile.username)),
            overlay: false,
        },
    )
    .await?;

    stream.flush().await?;
    sustain_play_connection(stream).await
}

async fn send_initial_chunks(stream: &mut TcpStream, view_distance: i32) -> anyhow::Result<()> {
    let radius = view_distance.clamp(0, 1);
    let mut count = 0;

    send_packet(
        stream,
        &to_client::play::chunk_batch_start::ChunkBatchStart {},
    )
    .await?;
    for chunk_z in -radius..=radius {
        for chunk_x in -radius..=radius {
            let chunk = crate::world::empty_chunk_packet(chunk_x, chunk_z)?;
            send_packet(stream, &chunk).await?;
            count += 1;
        }
    }
    send_packet(
        stream,
        &to_client::play::chunk_batch_finished::ChunkBatchFinished {
            batch_size: VarInt(count),
        },
    )
    .await
}

async fn sustain_play_connection(stream: &mut TcpStream) -> anyhow::Result<()> {
    let (mut reader, mut writer) = stream.split();
    let mut keep_alive = tokio::time::interval(std::time::Duration::from_secs(10));
    keep_alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut keep_alive_id = 0_i64;

    loop {
        tokio::select! {
            frame = read_frame(&mut reader) => {
                let mut payload = frame?;
                let packet_id = read_packet_id(&mut payload)?;
                if packet_id == qexed_protocol::to_server::play::keep_alive::KeepAlive::ID {
                    let _packet =
                        decode_payload::<qexed_protocol::to_server::play::keep_alive::KeepAlive>(
                            &mut payload,
                        )?;
                }
            }
            _ = keep_alive.tick() => {
                keep_alive_id = keep_alive_id.wrapping_add(1);
                send_packet(
                    &mut writer,
                    &to_client::play::keep_alive::KeepAlive { keep_alive_id },
                )
                .await?;
                writer.flush().await?;
            }
        }
    }
}

async fn send_login_disconnect(
    stream: &mut TcpStream,
    reason: impl Into<String>,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        &to_client::login::disconnect::Disconnect {
            reason: JsonValue(serde_json::json!({ "text": reason.into() })),
        },
    )
    .await
}

fn offline_profile(login_start: LoginStart) -> GameProfile {
    let uuid = if login_start.player_uuid == uuid::Uuid::nil() {
        uuid::Uuid::new_v3(
            &uuid::Uuid::NAMESPACE_DNS,
            format!("OfflinePlayer:{}", login_start.username).as_bytes(),
        )
    } else {
        login_start.player_uuid
    };

    GameProfile {
        uuid,
        username: login_start.username,
        properties: Vec::new(),
    }
}

fn server_session_id() -> uuid::Uuid {
    *SERVER_SESSION_ID.get_or_init(uuid::Uuid::new_v4)
}

async fn read_expected_packet<T>(stream: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<T>
where
    T: Packet + Default,
{
    let mut payload = read_frame(stream).await?;
    let packet_id = read_packet_id(&mut payload)?;
    if packet_id != T::ID {
        anyhow::bail!(
            "packet ID mismatch: expected {}, actual {}",
            T::ID,
            packet_id
        );
    }
    decode_payload::<T>(&mut payload)
}

fn decode_payload<T>(payload: &mut BytesMut) -> anyhow::Result<T>
where
    T: Packet + Default,
{
    let mut reader = PacketReader::new(payload);
    let mut packet = T::default();
    packet.deserialize(&mut reader)?;
    Ok(packet)
}

fn read_packet_id(payload: &mut BytesMut) -> anyhow::Result<i32> {
    let mut reader = PacketReader::new(payload);
    let mut packet_id = VarInt::default();
    packet_id.deserialize(&mut reader)?;
    Ok(packet_id.0)
}

async fn send_packet<T>(stream: &mut (impl AsyncWrite + Unpin), packet: &T) -> anyhow::Result<()>
where
    T: Packet,
{
    let payload = build_payload(T::ID, packet)?;
    write_frame(stream, &payload).await
}

fn build_payload<T>(packet_id: i32, packet: &T) -> anyhow::Result<BytesMut>
where
    T: Packet,
{
    let mut payload = BytesMut::new();
    {
        let mut writer = PacketWriter::new(&mut payload);
        VarInt(packet_id).serialize(&mut writer)?;
        packet.serialize(&mut writer)?;
    }
    Ok(payload)
}

async fn read_frame(stream: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<BytesMut> {
    let length = read_varint_async(stream).await?;
    if length < 0 {
        anyhow::bail!("negative packet length: {length}");
    }

    let length = length as usize;
    if length > MAX_PACKET_SIZE {
        anyhow::bail!("packet length {length} exceeds max {MAX_PACKET_SIZE}");
    }

    let mut payload = BytesMut::zeroed(length);
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

async fn write_frame(stream: &mut (impl AsyncWrite + Unpin), payload: &[u8]) -> anyhow::Result<()> {
    if payload.len() > MAX_PACKET_SIZE {
        anyhow::bail!(
            "packet length {} exceeds max {MAX_PACKET_SIZE}",
            payload.len()
        );
    }

    let mut header = BytesMut::new();
    write_varint(&mut header, payload.len() as i32);
    stream.write_all(&header).await?;
    stream.write_all(payload).await?;
    Ok(())
}

async fn read_varint_async(stream: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<i32> {
    let mut value = 0_i32;
    for position in 0..5 {
        let byte = stream.read_u8().await?;
        value |= ((byte & 0x7F) as i32) << (7 * position);
        if (byte & 0x80) == 0 {
            return Ok(value);
        }
    }
    anyhow::bail!("VarInt is too large")
}

fn write_varint(buf: &mut BytesMut, value: i32) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if value == 0 {
            break;
        }
    }
}

fn string_payload(value: &str) -> anyhow::Result<Vec<u8>> {
    let mut payload = BytesMut::new();
    let mut writer = PacketWriter::new(&mut payload);
    value.to_string().serialize(&mut writer)?;
    Ok(payload.to_vec())
}

fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

#[cfg(test)]
mod tests {
    use super::{
        build_payload, decode_payload, offline_profile, read_frame, read_packet_id, write_frame,
        write_varint,
    };
    use bytes::BytesMut;
    use qexed_packet::{Packet, net_types::VarInt};
    use qexed_protocol::to_client;
    use qexed_protocol::to_client::status::ping::Ping;
    use qexed_protocol::to_client::status::server_info::ServerInfo;
    use qexed_protocol::to_server::configuration::{
        finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
        select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
    };
    use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
    use qexed_protocol::to_server::login::{
        login_acknowledged::LoginAcknowledged, login_start::LoginStart,
    };
    use qexed_protocol::to_server::status::ping_start::PingStart;
    use tokio::net::{TcpListener, TcpStream};

    #[test]
    fn packet_payload_starts_with_packet_id() {
        let payload = build_payload(1, &Ping { time: 42 }).unwrap();

        assert_eq!(payload[0], 1);
    }

    #[test]
    fn varint_writer_uses_minecraft_encoding() {
        let mut buf = BytesMut::new();
        write_varint(&mut buf, 300);

        assert_eq!(buf.as_ref(), &[0xac, 0x02]);
    }

    #[test]
    fn offline_profile_uses_client_uuid_when_present() {
        let uuid = uuid::Uuid::from_u128(1);
        let profile = offline_profile(LoginStart {
            username: "Steve".to_string(),
            player_uuid: uuid,
        });

        assert_eq!(profile.uuid, uuid);
    }

    #[tokio::test]
    async fn status_handshake_responds_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = qexed_config::app::qexed::Qexed::default();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        write_frame(
            &mut client,
            &build_payload(
                SetProtocol::ID,
                &SetProtocol {
                    protocol_version: VarInt(qexed_config::PROTOCOL_VERSION),
                    server_host: "localhost".to_string(),
                    server_port: 25565,
                    next_state: VarInt(1),
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();
        write_frame(
            &mut client,
            &build_payload(PingStart::ID, &PingStart {}).unwrap(),
        )
        .await
        .unwrap();

        let mut payload = read_frame(&mut client).await.unwrap();
        assert_eq!(read_packet_id(&mut payload).unwrap(), ServerInfo::ID);
        let response = decode_payload::<ServerInfo>(&mut payload).unwrap();

        assert_eq!(
            response.response.0["version"]["protocol"],
            qexed_config::PROTOCOL_VERSION
        );
        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn login_reaches_play_and_sends_initial_chunks() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = qexed_config::app::qexed::Qexed::default();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        write_packet(
            &mut client,
            &SetProtocol {
                protocol_version: VarInt(qexed_config::PROTOCOL_VERSION),
                server_host: "localhost".to_string(),
                server_port: 25565,
                next_state: VarInt(2),
            },
        )
        .await;
        write_packet(
            &mut client,
            &LoginStart {
                username: "Steve".to_string(),
                player_uuid: uuid::Uuid::from_u128(1),
            },
        )
        .await;

        assert_eq!(
            read_next_packet_id(&mut client).await,
            to_client::login::success::Success::ID
        );
        write_packet(&mut client, &LoginAcknowledged {}).await;
        wait_for_clientbound_config_select_known_packs(&mut client).await;
        write_packet(
            &mut client,
            &ServerboundSelectKnownPacks {
                entries: qexed_registry::known_packs(),
            },
        )
        .await;
        wait_for_clientbound_finish_configuration(&mut client).await;
        write_packet(&mut client, &ServerboundFinishConfiguration {}).await;

        let mut saw_login = false;
        let mut saw_position = false;
        let mut saw_player_info = false;
        let mut saw_load_start = false;
        let mut saw_batch_start = false;
        let mut map_chunks = 0;
        let mut saw_batch_finished = false;

        for _ in 0..64 {
            let packet_id = read_next_packet_id(&mut client).await;
            match packet_id {
                to_client::play::login::Login::ID => saw_login = true,
                to_client::play::position::Position::ID => saw_position = true,
                to_client::play::player_info_update::PlayerInfoUpdate::ID => saw_player_info = true,
                to_client::play::game_state_change::GameStateChange::ID => saw_load_start = true,
                to_client::play::chunk_batch_start::ChunkBatchStart::ID => saw_batch_start = true,
                to_client::play::map_chunk::MapChunk::ID => map_chunks += 1,
                to_client::play::chunk_batch_finished::ChunkBatchFinished::ID => {
                    saw_batch_finished = true;
                    break;
                }
                _ => {}
            }
        }

        assert!(saw_login, "play login packet was not sent");
        assert!(saw_position, "position packet was not sent");
        assert!(saw_player_info, "player info update packet was not sent");
        assert!(
            saw_load_start,
            "level chunks load start packet was not sent"
        );
        assert!(saw_batch_start, "chunk batch start packet was not sent");
        assert!(map_chunks > 0, "initial map chunks were not sent");
        assert!(
            saw_batch_finished,
            "chunk batch finished packet was not sent"
        );

        drop(client);
        server.await.unwrap();
    }

    async fn write_packet<T>(stream: &mut TcpStream, packet: &T)
    where
        T: Packet,
    {
        write_frame(stream, &build_payload(T::ID, packet).unwrap())
            .await
            .unwrap();
    }

    async fn read_next_packet_id(stream: &mut TcpStream) -> i32 {
        let mut payload = read_frame(stream).await.unwrap();
        read_packet_id(&mut payload).unwrap()
    }

    async fn wait_for_clientbound_config_select_known_packs(stream: &mut TcpStream) {
        loop {
            let packet_id = read_next_packet_id(stream).await;
            if packet_id == to_client::configuration::select_known_packs::SelectKnownPacks::ID {
                return;
            }
        }
    }

    async fn wait_for_clientbound_finish_configuration(stream: &mut TcpStream) {
        loop {
            let packet_id = read_next_packet_id(stream).await;
            if packet_id == to_client::configuration::finish_configuration::FinishConfiguration::ID
            {
                return;
            }
        }
    }

    fn init_registry_for_connection_test() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let config_path = std::env::temp_dir().join("qexed-v5-connection-test-config");
            let _ = qexed_config::CONFIG_PATH.set(config_path);
            qexed_registry::init().expect("registry must initialize for connection tests");
        });
    }
}
