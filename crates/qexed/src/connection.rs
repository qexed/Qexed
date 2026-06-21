use bytes::BytesMut;
use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{ByteArray, GameProfile, JsonValue, RestBuffer, VarInt},
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
        login::{
            encryption_begin::EncryptionBegin as ServerboundEncryptionBegin,
            login_acknowledged::LoginAcknowledged, login_start::LoginStart,
        },
        status::{ping::Ping as ServerboundPing, ping_start::PingStart},
    },
};
use qexed_tcp_connect::{FramePart, PacketReadError, PacketSink, PacketStream, PacketWriteError};
use std::collections::HashSet;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    time::{Duration, timeout},
};

const SERVER_BRAND: &str = "qexed";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);
const CONFIGURATION_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const INITIAL_CHUNK_RADIUS: i32 = 0;
const CHUNK_SYNC_BATCH_LIMIT: usize = 4;
const CHUNK_SYNC_INTERVAL: Duration = Duration::from_secs(1);
static SERVER_SESSION_ID: std::sync::OnceLock<uuid::Uuid> = std::sync::OnceLock::new();

type ClientConnection = ConnectionIo<OwnedReadHalf, OwnedWriteHalf>;

#[derive(Debug, Clone)]
struct LoginSession {
    profile: GameProfile,
    online_mode: bool,
}

#[derive(Debug)]
struct ConnectionClosed;

impl std::fmt::Display for ConnectionClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("connection closed")
    }
}

impl std::error::Error for ConnectionClosed {}

struct ConnectionIo<R, W> {
    reader: PacketStream<R>,
    writer: PacketSink<W>,
}

impl<R, W> ConnectionIo<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader: PacketStream::new(reader),
            writer: PacketSink::new(writer),
        }
    }

    async fn read_frame(&mut self) -> anyhow::Result<BytesMut> {
        read_frame(&mut self.reader).await
    }

    async fn read_frame_with_timeout(
        &mut self,
        duration: Duration,
        operation: &'static str,
    ) -> anyhow::Result<BytesMut> {
        with_timeout(duration, operation, self.read_frame()).await
    }

    async fn read_expected_packet<T>(&mut self) -> anyhow::Result<T>
    where
        T: Packet + Default,
    {
        read_expected_packet(&mut self.reader).await
    }

    async fn read_expected_packet_with_timeout<T>(
        &mut self,
        duration: Duration,
        operation: &'static str,
    ) -> anyhow::Result<T>
    where
        T: Packet + Default,
    {
        with_timeout(duration, operation, self.read_expected_packet::<T>()).await
    }

    async fn send_packet<T>(&mut self, packet: &T) -> anyhow::Result<()>
    where
        T: Packet,
    {
        with_timeout(
            WRITE_TIMEOUT,
            "send packet",
            send_packet(&mut self.writer, packet),
        )
        .await
    }

    async fn send_packet_batch(
        &mut self,
        build: impl FnOnce(&mut PacketBatch<'_, W>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.send_packet_batch_with_capacity(0, build).await
    }

    async fn send_packet_batch_with_capacity(
        &mut self,
        capacity: usize,
        build: impl FnOnce(&mut PacketBatch<'_, W>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let frames = {
            let mut batch = PacketBatch::with_capacity(&self.writer, capacity);
            build(&mut batch)?;
            batch.into_frames()
        };
        with_timeout(
            WRITE_TIMEOUT,
            "send packet batch",
            self.writer.send_encoded_frames_vectored(&frames),
        )
        .await?;
        Ok(())
    }

    async fn flush(&mut self) -> anyhow::Result<()> {
        with_timeout(WRITE_TIMEOUT, "flush connection", self.writer.flush()).await?;
        Ok(())
    }

    fn enable_encryption(&mut self, shared_secret: &[u8]) -> anyhow::Result<()> {
        self.reader.enable_encryption(shared_secret)?;
        self.writer.enable_encryption(shared_secret)?;
        Ok(())
    }
}

struct PacketBatch<'a, W> {
    sink: &'a PacketSink<W>,
    frames: Vec<FramePart>,
    current: BytesMut,
}

impl<'a, W> PacketBatch<'a, W>
where
    W: AsyncWrite + Unpin,
{
    fn with_capacity(sink: &'a PacketSink<W>, capacity: usize) -> Self {
        Self {
            sink,
            frames: Vec::new(),
            current: BytesMut::with_capacity(capacity),
        }
    }

    fn push<T>(&mut self, packet: &T) -> anyhow::Result<()>
    where
        T: Packet,
    {
        self.sink
            .append_packet_frame_ref(packet, &mut self.current)?;
        Ok(())
    }

    fn flush_current(&mut self) {
        if self.current.is_empty() {
            return;
        }

        let current = std::mem::take(&mut self.current);
        self.frames.push(FramePart::Bytes(current.freeze()));
    }

    fn into_frames(mut self) -> Vec<FramePart> {
        self.flush_current();
        self.frames
    }
}

pub async fn handle(stream: TcpStream, config: crate::bootstrap::RuntimeConfig) {
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
    if err.downcast_ref::<ConnectionClosed>().is_some() {
        return true;
    }

    if let Some(err) = err.downcast_ref::<PacketReadError>() {
        return match err {
            PacketReadError::ConnectionClosedWithIncompletePacket => true,
            PacketReadError::OtherError(err) => is_expected_io_error(err),
            _ => false,
        };
    }

    if let Some(err) = err.downcast_ref::<PacketWriteError>() {
        return match err {
            PacketWriteError::OtherError(err) => is_expected_io_error(err),
            _ => false,
        };
    }

    err.downcast_ref::<std::io::Error>()
        .is_some_and(is_expected_io_error)
}

fn is_expected_io_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
    )
}

async fn handle_inner(
    stream: TcpStream,
    config: &crate::bootstrap::RuntimeConfig,
) -> anyhow::Result<()> {
    let peer_ip = stream.peer_addr().ok().map(|addr| addr.ip());
    let (reader, writer) = stream.into_split();
    let mut connection = ClientConnection::new(reader, writer);
    let handshake = connection
        .read_expected_packet_with_timeout::<SetProtocol>(
            HANDSHAKE_TIMEOUT,
            "read handshake packet",
        )
        .await?;
    match handshake.next_state.0 {
        1 => handle_status(&mut connection, config).await,
        2 | 3 => handle_login(&mut connection, config, handshake, peer_ip).await,
        state => anyhow::bail!("unsupported handshake target state: {state}"),
    }
}

async fn handle_status(
    connection: &mut ClientConnection,
    config: &crate::bootstrap::RuntimeConfig,
) -> anyhow::Result<()> {
    connection
        .read_expected_packet_with_timeout::<PingStart>(
            STATUS_TIMEOUT,
            "read status ping start packet",
        )
        .await?;

    connection
        .send_packet(&to_client::status::server_info::ServerInfo {
            response: JsonValue(serde_json::json!({
                "version": {
                    "name": qexed_config::MC_VERSION,
                    "protocol": qexed_config::PROTOCOL_VERSION,
                },
                "players": {
                    "max": config.qexed.server.max_players,
                    "online": 0,
                },
                "description": {
                    "text": config.qexed.server.motd,
                },
                "enforcesSecureChat": config.authenticator.is_online_mode(),
            })),
        })
        .await?;

    if let Ok(ping) = connection
        .read_expected_packet_with_timeout::<ServerboundPing>(
            STATUS_TIMEOUT,
            "read status ping packet",
        )
        .await
    {
        connection
            .send_packet(&to_client::status::ping::Ping { time: ping.time })
            .await?;
    }

    Ok(())
}

async fn handle_login(
    connection: &mut ClientConnection,
    config: &crate::bootstrap::RuntimeConfig,
    handshake: SetProtocol,
    peer_ip: Option<std::net::IpAddr>,
) -> anyhow::Result<()> {
    if handshake.protocol_version.0 != qexed_config::PROTOCOL_VERSION {
        send_login_disconnect(
            connection,
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

    let login_start = connection
        .read_expected_packet_with_timeout::<LoginStart>(LOGIN_TIMEOUT, "read login start packet")
        .await?;
    let session =
        match authenticate_login(connection, &config.authenticator, login_start, peer_ip).await {
            Ok(session) => session,
            Err(err) => {
                send_login_disconnect(connection, format!("正版验证失败: {err:#}")).await?;
                return Ok(());
            }
        };

    connection
        .send_packet(&to_client::login::success::Success {
            game_profile: session.profile.clone(),
            session_id: server_session_id(),
        })
        .await?;

    connection
        .read_expected_packet_with_timeout::<LoginAcknowledged>(
            LOGIN_TIMEOUT,
            "read login acknowledged packet",
        )
        .await?;
    handle_configuration(connection).await?;
    enter_play(
        connection,
        config,
        config.authenticator.clone(),
        config.chat.clone(),
        &session,
    )
    .await
}

async fn authenticate_login(
    connection: &mut ClientConnection,
    auth: &qexed_auth::Authenticator,
    login_start: LoginStart,
    peer_ip: Option<std::net::IpAddr>,
) -> anyhow::Result<LoginSession> {
    if !auth.is_online_mode() {
        let session = auth.authenticate_offline(&login_start.username, login_start.player_uuid);
        return Ok(LoginSession {
            profile: session.into_profile(),
            online_mode: false,
        });
    }

    let verify_token = auth.new_verify_token();
    connection
        .send_packet(&to_client::login::encryption_begin::EncryptionBegin {
            server_id: String::new(),
            public_key: ByteArray(auth.public_key_der()?),
            verify_token: ByteArray(verify_token.clone()),
            should_authenticate: true,
        })
        .await?;

    let key_packet = connection
        .read_expected_packet_with_timeout::<ServerboundEncryptionBegin>(
            LOGIN_TIMEOUT,
            "read login encryption response packet",
        )
        .await?;
    let shared_secret = auth.decrypt_login_key(&key_packet, &verify_token).await?;
    connection.enable_encryption(&shared_secret)?;

    let session = auth
        .authenticate_online(&login_start.username, &shared_secret, peer_ip)
        .await?;
    Ok(LoginSession {
        profile: session.into_profile(),
        online_mode: true,
    })
}

async fn handle_configuration(connection: &mut ClientConnection) -> anyhow::Result<()> {
    connection
        .send_packet_batch(|batch| {
            batch.push(&to_client::configuration::custom_payload::CustomPayload {
                channel: "minecraft:brand".to_string(),
                data: RestBuffer(string_payload(SERVER_BRAND)?),
            })?;
            batch.push(&to_client::configuration::feature_flags::FeatureFlags {
                features: vec![qexed_registry::VANILLA_FEATURE.to_string()],
            })?;
            batch.push(
                &to_client::configuration::select_known_packs::SelectKnownPacks {
                    known_packs: qexed_registry::known_packs(),
                },
            )
        })
        .await?;

    let selected_packs = wait_for_known_packs(connection).await?;
    let include_contents = !qexed_registry::accepts_vanilla_core_pack(&selected_packs.entries);
    let registry_packets = qexed_registry::load_registry_packets(include_contents)?;
    let tags = qexed_registry::load_tag_packet()?;

    connection
        .send_packet_batch_with_capacity(64 * 1024, |batch| {
            for packet in &registry_packets {
                batch.push(packet)?;
            }
            batch.push(&tags)?;
            batch.push(&to_client::configuration::finish_configuration::FinishConfiguration {})
        })
        .await?;

    wait_for_finish_configuration(connection).await
}

async fn wait_for_known_packs(
    connection: &mut ClientConnection,
) -> anyhow::Result<ServerboundSelectKnownPacks> {
    loop {
        let mut payload = connection
            .read_frame_with_timeout(
                CONFIGURATION_TIMEOUT,
                "read configuration known packs packet",
            )
            .await?;
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundSelectKnownPacks::ID {
            return decode_payload::<ServerboundSelectKnownPacks>(&mut payload);
        }
        if handle_configuration_side_packet(connection, packet_id, &mut payload).await? {
            continue;
        }
        tklog::debug!(format!(
            "skip configuration packet while waiting known packs: {packet_id}"
        ));
    }
}

async fn wait_for_finish_configuration(connection: &mut ClientConnection) -> anyhow::Result<()> {
    loop {
        let mut payload = connection
            .read_frame_with_timeout(CONFIGURATION_TIMEOUT, "read finish configuration packet")
            .await?;
        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundFinishConfiguration::ID {
            let _packet = decode_payload::<ServerboundFinishConfiguration>(&mut payload)?;
            return Ok(());
        }
        if handle_configuration_side_packet(connection, packet_id, &mut payload).await? {
            continue;
        }
        tklog::debug!(format!(
            "skip configuration packet while waiting finish: {packet_id}"
        ));
    }
}

async fn handle_configuration_side_packet(
    connection: &mut ClientConnection,
    packet_id: i32,
    payload: &mut BytesMut,
) -> anyhow::Result<bool> {
    if packet_id == ServerboundSettings::ID {
        let _settings = decode_payload::<ServerboundSettings>(payload)?;
        return Ok(true);
    }
    if packet_id
        == qexed_protocol::to_server::configuration::client_information::ClientInformation::ID
    {
        let _packet = decode_payload::<
            qexed_protocol::to_server::configuration::client_information::ClientInformation,
        >(payload)?;
        return Ok(true);
    }
    if packet_id == qexed_protocol::to_server::configuration::custom_payload::CustomPayload::ID {
        let _packet = decode_payload::<
            qexed_protocol::to_server::configuration::custom_payload::CustomPayload,
        >(payload)?;
        return Ok(true);
    }
    if packet_id
        == qexed_protocol::to_server::configuration::resource_pack_receive::ResourcePackReceive::ID
    {
        let _packet = decode_payload::<
            qexed_protocol::to_server::configuration::resource_pack_receive::ResourcePackReceive,
        >(payload)?;
        return Ok(true);
    }
    if packet_id == qexed_protocol::to_server::configuration::keep_alive::KeepAlive::ID {
        let packet = decode_payload::<
            qexed_protocol::to_server::configuration::keep_alive::KeepAlive,
        >(payload)?;
        connection
            .send_packet(&to_client::configuration::keep_alive::KeepAlive {
                keep_alive_id: packet.keep_alive_id,
            })
            .await?;
        return Ok(true);
    }
    Ok(false)
}

async fn enter_play(
    connection: &mut ClientConnection,
    runtime: &crate::bootstrap::RuntimeConfig,
    authenticator: qexed_auth::Authenticator,
    chat_service: qexed_chat::ChatService,
    session: &LoginSession,
) -> anyhow::Result<()> {
    let config = &runtime.qexed;
    let profile = &session.profile;
    let view_distance = config.server.view_distance.max(1);
    let simulation_distance = config.server.simulation_distance.max(1);
    let dimension_type = qexed_registry::dimension_type_holder_id("minecraft:overworld")?;

    connection
        .send_packet_batch_with_capacity(initial_play_batch_capacity(view_distance), |batch| {
            batch.push(&to_client::play::login::Login {
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
                online_mode: session.online_mode,
                enforces_secure_chat: session.online_mode,
            })?;
            batch.push(&to_client::play::position::Position {
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
            })?;
            batch.push(&to_client::play::change_difficulty::ChangeDifficulty {
                difficulty: 2,
                locked: false,
            })?;
            batch.push(&to_client::play::set_held_slot::SetHeldSlot { slot: VarInt(0) })?;
            batch.push(&to_client::play::server_data::ServerData {
                motd: text_component(config.server.motd.clone()),
                icon_bytes: None,
            })?;
            batch.push(&to_client::play::player_info_update::PlayerInfoUpdate {
                actions:
                    to_client::play::player_info_update::PlayerInfoActions::player_initializing(),
                entries: vec![
                    to_client::play::player_info_update::PlayerInfoEntry::from_profile(profile, 1),
                ],
            })?;
            batch.push(&to_client::play::player_abilities::PlayerAbilities {
                flags: to_client::play::player_abilities::PlayerAbilities::INVULNERABLE
                    | to_client::play::player_abilities::PlayerAbilities::CAN_FLY
                    | to_client::play::player_abilities::PlayerAbilities::INSTABUILD,
                flying_speed: 0.05,
                walking_speed: 0.1,
            })?;
            batch.push(&to_client::play::set_health::SetHealth {
                health: 20.0,
                food: VarInt(20),
                saturation: 5.0,
            })?;
            batch.push(&to_client::play::set_experience::SetExperience {
                experience_progress: 0.0,
                experience_level: VarInt(0),
                total_experience: VarInt(0),
            })?;
            batch.push(&to_client::play::initialize_border::InitializeBorder::default())?;
            batch.push(&to_client::play::set_time::SetTime {
                game_time: 0,
                clock_updates: Vec::new(),
            })?;
            batch.push(
                &to_client::play::set_default_spawn_position::SetDefaultSpawnPosition {
                    dimension: "minecraft:overworld".to_string(),
                    position: qexed_packet::net_types::Position { x: 0, y: 64, z: 0 },
                    yaw: 0.0,
                    pitch: 0.0,
                },
            )?;
            batch.push(&to_client::play::game_state_change::GameStateChange {
                reason: 13,
                game_mode: 0.0,
            })?;
            batch.push(&qexed_command::command_tree())?;
            batch.push(
                &to_client::play::set_simulation_distance::SetSimulationDistance {
                    simulation_distance: VarInt(simulation_distance),
                },
            )?;
            batch.push(&to_client::play::update_view_distance::UpdateViewDistance {
                view_distance: VarInt(view_distance),
            })?;
            batch.push(&to_client::play::update_view_position::UpdateViewPosition {
                chunk_x: VarInt(0),
                chunk_z: VarInt(0),
            })?;
            batch.push(&to_client::play::ticking_state::TickingState::default())?;
            batch.push(&to_client::play::system_chat::SystemChat {
                content: text_component(format!("{} joined qexed-v5", profile.username)),
                overlay: false,
            })
        })
        .await?;

    let mut chunk_view = PlayerChunkView::new(view_distance);
    send_chunk_window(
        connection,
        runtime,
        &mut chunk_view,
        0,
        0,
        INITIAL_CHUNK_RADIUS,
        None,
        qexed_world::ChunkSyncCause::InitialLogin,
    )
    .await?;
    connection.flush().await?;
    let command_context = command_context(config, profile);
    let chat_context = ChatContext {
        profile: profile.clone(),
        online_mode: session.online_mode,
        authenticator,
        service: chat_service,
    };
    sustain_play_connection(
        connection,
        runtime,
        command_context,
        chat_context,
        chunk_view,
    )
    .await
}

fn initial_play_batch_capacity(view_distance: i32) -> usize {
    let radius = chunk_view_radius(view_distance) as usize;
    let chunk_count = (radius * 2 + 1).pow(2);
    8 * 1024 + chunk_count * 16
}

#[derive(Debug, Clone)]
struct PlayerChunkView {
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
    sent_chunks: HashSet<(i32, i32)>,
}

impl PlayerChunkView {
    fn new(view_distance: i32) -> Self {
        Self {
            center_chunk_x: 0,
            center_chunk_z: 0,
            radius: chunk_view_radius(view_distance),
            sent_chunks: HashSet::new(),
        }
    }

    fn is_complete(&self) -> bool {
        self.sent_chunks.len() >= chunk_window_len(self.radius)
    }
}

fn chunk_view_radius(view_distance: i32) -> i32 {
    view_distance.max(1)
}

fn chunk_window_len(radius: i32) -> usize {
    let width = (radius * 2 + 1).max(1) as usize;
    width * width
}

fn player_chunk_coordinate(position: f64) -> i32 {
    (position.floor() as i32).div_euclid(16)
}

async fn sync_player_chunk_position(
    connection: &mut ClientConnection,
    runtime: &crate::bootstrap::RuntimeConfig,
    chunk_view: &mut PlayerChunkView,
    x: f64,
    z: f64,
) -> anyhow::Result<()> {
    let chunk_x = player_chunk_coordinate(x);
    let chunk_z = player_chunk_coordinate(z);
    if chunk_x == chunk_view.center_chunk_x
        && chunk_z == chunk_view.center_chunk_z
        && chunk_view.is_complete()
    {
        return Ok(());
    }

    send_chunk_window(
        connection,
        runtime,
        chunk_view,
        chunk_x,
        chunk_z,
        chunk_view.radius,
        Some(CHUNK_SYNC_BATCH_LIMIT),
        qexed_world::ChunkSyncCause::PlayerMove,
    )
    .await
}

async fn send_chunk_window(
    connection: &mut ClientConnection,
    runtime: &crate::bootstrap::RuntimeConfig,
    chunk_view: &mut PlayerChunkView,
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
    max_new_chunks: Option<usize>,
    cause: qexed_world::ChunkSyncCause,
) -> anyhow::Result<()> {
    let mut count = 0;
    let dimension = qexed_save::DimensionId::overworld();

    let mut target_chunks = chunk_window_positions(center_chunk_x, center_chunk_z, radius);
    let full_window = chunk_window_positions(center_chunk_x, center_chunk_z, chunk_view.radius);
    let mut chunks = Vec::new();
    let mut loaded = Vec::new();

    if radius == chunk_view.radius {
        target_chunks.extend(
            chunk_view
                .sent_chunks
                .iter()
                .copied()
                .filter(|chunk| full_window.contains(chunk)),
        );
    }

    let mut candidates = target_chunks
        .iter()
        .copied()
        .filter(|chunk| !chunk_view.sent_chunks.contains(chunk))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(chunk_x, chunk_z)| {
        let dx = chunk_x - center_chunk_x;
        let dz = chunk_z - center_chunk_z;
        dx * dx + dz * dz
    });

    for (chunk_x, chunk_z) in candidates {
        if max_new_chunks.is_some_and(|limit| count >= limit) {
            target_chunks.remove(&(chunk_x, chunk_z));
            continue;
        }
        let worldgen_client = None;
        let load = runtime
            .world
            .ensure_network_chunk_with_event(
                runtime.local_worldgen.as_deref(),
                worldgen_client,
                &dimension,
                chunk_x,
                chunk_z,
            )
            .await?;
        loaded.push(qexed_world::ChunkLoadEvent::new(
            chunk_x,
            chunk_z,
            load.source,
        ));
        chunks.push(load.chunk);
        count += 1;
    }
    let leaving_chunks = chunk_view
        .sent_chunks
        .difference(&full_window)
        .copied()
        .collect::<Vec<_>>();
    let unloaded = leaving_chunks
        .iter()
        .map(|(chunk_x, chunk_z)| qexed_world::ChunkUnloadEvent::new(*chunk_x, *chunk_z))
        .collect::<Vec<_>>();
    let sync_event =
        qexed_world::ChunkSyncEvent::new(cause, center_chunk_x, center_chunk_z, loaded, unloaded);

    chunk_view.center_chunk_x = center_chunk_x;
    chunk_view.center_chunk_z = center_chunk_z;
    chunk_view.sent_chunks = target_chunks;

    connection
        .send_packet_batch_with_capacity(256 * 1024, |batch| {
            batch.push(&to_client::play::update_view_position::UpdateViewPosition {
                chunk_x: VarInt(center_chunk_x),
                chunk_z: VarInt(center_chunk_z),
            })?;
            if count > 0 {
                batch.push(&to_client::play::chunk_batch_start::ChunkBatchStart {})?;
                for chunk in &chunks {
                    batch.push(chunk)?;
                }
                batch.push(&to_client::play::chunk_batch_finished::ChunkBatchFinished {
                    batch_size: VarInt(count as i32),
                })?;
            }
            for (chunk_x, chunk_z) in &leaving_chunks {
                batch.push(&to_client::play::forget_level_chunk::ForgetLevelChunk {
                    chunk_x: *chunk_x,
                    chunk_z: *chunk_z,
                })?;
            }
            Ok(())
        })
        .await?;
    log_chunk_sync_event(&sync_event);
    Ok(())
}

fn log_chunk_sync_event(event: &qexed_world::ChunkSyncEvent) {
    tklog::debug!(format!(
        "synced player chunks: cause={:?}, center=({}, {}), sent_new={}, unloaded={}, first_loaded={}, first_unloaded={}, sources={}",
        event.cause,
        event.center_chunk_x,
        event.center_chunk_z,
        event.loaded_count(),
        event.unloaded_count(),
        first_loaded_chunk(event),
        first_unloaded_chunk(event),
        chunk_source_summary(event)
    ));
}

fn first_loaded_chunk(event: &qexed_world::ChunkSyncEvent) -> String {
    event
        .loaded
        .first()
        .map(|load| format!("({}, {})", load.chunk_x, load.chunk_z))
        .unwrap_or_else(|| "none".to_string())
}

fn first_unloaded_chunk(event: &qexed_world::ChunkSyncEvent) -> String {
    event
        .unloaded
        .first()
        .map(|unload| format!("({}, {})", unload.chunk_x, unload.chunk_z))
        .unwrap_or_else(|| "none".to_string())
}

fn chunk_source_summary(event: &qexed_world::ChunkSyncEvent) -> String {
    let counts = event.source_counts();
    format!(
        "saved={}, local_generated={}, vanilla_generated={}, empty_fallback={}",
        counts.saved, counts.local_generated, counts.vanilla_generated, counts.empty_fallback
    )
}

fn chunk_window_positions(
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
) -> HashSet<(i32, i32)> {
    let mut chunks = HashSet::new();
    for chunk_z in center_chunk_z - radius..=center_chunk_z + radius {
        for chunk_x in center_chunk_x - radius..=center_chunk_x + radius {
            chunks.insert((chunk_x, chunk_z));
        }
    }
    chunks
}

fn command_context(
    config: &qexed_config::app::qexed::Qexed,
    profile: &GameProfile,
) -> qexed_command::CommandContext {
    qexed_command::CommandContext {
        locale: "zh_cn".to_string(),
        online_players: vec![profile.username.clone()],
        max_players: config.server.max_players,
        version: version_message(),
        plugins: plugin_summaries(),
    }
}

fn version_message() -> String {
    format!(
        "qexed {} (Minecraft {}, protocol {})",
        env!("CARGO_PKG_VERSION"),
        qexed_config::MC_VERSION,
        qexed_config::PROTOCOL_VERSION
    )
}

fn plugin_summaries() -> Vec<String> {
    qexed_plugin::try_plugin_manager()
        .map(qexed_plugin::PluginManager::plugin_summaries)
        .unwrap_or_default()
}

async fn sustain_play_connection(
    connection: &mut ClientConnection,
    runtime: &crate::bootstrap::RuntimeConfig,
    command_context: qexed_command::CommandContext,
    chat_context: ChatContext,
    mut chunk_view: PlayerChunkView,
) -> anyhow::Result<()> {
    let mut keep_alive = tokio::time::interval(std::time::Duration::from_secs(10));
    keep_alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut chunk_sync = tokio::time::interval(CHUNK_SYNC_INTERVAL);
    chunk_sync.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut keep_alive_id = 0_i64;
    let mut chat_session: Option<qexed_chat::SecureChatSession> = None;
    let mut next_chat_global_index = 0_i32;

    loop {
        tokio::select! {
            frame = read_frame(&mut connection.reader) => {
                let mut payload = frame?;
                let packet_id = read_packet_id(&mut payload)?;
                if packet_id == qexed_protocol::to_server::play::keep_alive::KeepAlive::ID {
                    let _packet =
                        decode_payload::<qexed_protocol::to_server::play::keep_alive::KeepAlive>(
                            &mut payload,
                        )?;
                } else if packet_id == qexed_protocol::to_server::play::chat_ack::ChatAck::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::chat_ack::ChatAck>(
                            &mut payload,
                        )?;
                    if let Some(chat_session) = chat_session.as_mut() {
                        chat_session.apply_offset(packet.offset)?;
                    }
                } else if packet_id == qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate>(
                            &mut payload,
                        )?;
                    handle_chat_session_update(connection, &chat_context, &mut chat_session, packet).await?;
                } else if packet_id == qexed_protocol::to_server::play::chat_command::ChatCommand::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::chat_command::ChatCommand>(
                            &mut payload,
                        )?;
                    handle_chat_command(connection, &command_context, &packet.command).await?;
                } else if packet_id == qexed_protocol::to_server::play::chat_message::ChatMessage::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::chat_message::ChatMessage>(
                            &mut payload,
                        )?;
                    handle_chat_message(
                        connection,
                        &chat_context,
                        &mut chat_session,
                        &mut next_chat_global_index,
                        packet,
                    ).await?;
                } else if packet_id == qexed_protocol::to_server::play::command_suggestion::CommandSuggestion::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::command_suggestion::CommandSuggestion>(
                            &mut payload,
                        )?;
                    handle_command_suggestion(connection, &command_context, packet).await?;
                } else if packet_id == qexed_protocol::to_server::play::move_player_pos::MovePlayerPos::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::move_player_pos::MovePlayerPos>(
                            &mut payload,
                        )?;
                    sync_player_chunk_position(connection, runtime, &mut chunk_view, packet.x, packet.z).await?;
                } else if packet_id == qexed_protocol::to_server::play::move_player_pos_rot::MovePlayerPosRot::ID {
                    let packet =
                        decode_payload::<qexed_protocol::to_server::play::move_player_pos_rot::MovePlayerPosRot>(
                            &mut payload,
                        )?;
                    sync_player_chunk_position(connection, runtime, &mut chunk_view, packet.x, packet.z).await?;
                } else if packet_id == qexed_protocol::to_server::play::move_player_rot::MovePlayerRot::ID {
                    let _packet =
                        decode_payload::<qexed_protocol::to_server::play::move_player_rot::MovePlayerRot>(
                            &mut payload,
                        )?;
                } else if packet_id == qexed_protocol::to_server::play::move_player_status_only::MovePlayerStatusOnly::ID {
                    let _packet =
                        decode_payload::<qexed_protocol::to_server::play::move_player_status_only::MovePlayerStatusOnly>(
                            &mut payload,
                        )?;
                } else if packet_id == qexed_protocol::to_server::play::chunk_batch_received::ChunkBatchReceived::ID {
                    let _packet =
                        decode_payload::<qexed_protocol::to_server::play::chunk_batch_received::ChunkBatchReceived>(
                            &mut payload,
                        )?;
                }
            }
            _ = keep_alive.tick() => {
                keep_alive_id = keep_alive_id.wrapping_add(1);
                connection
                    .send_packet(&to_client::play::keep_alive::KeepAlive { keep_alive_id })
                    .await?;
                connection.flush().await?;
            }
            _ = chunk_sync.tick(), if !chunk_view.is_complete() => {
                let center_chunk_x = chunk_view.center_chunk_x;
                let center_chunk_z = chunk_view.center_chunk_z;
                let radius = chunk_view.radius;
                send_chunk_window(
                    connection,
                    runtime,
                    &mut chunk_view,
                    center_chunk_x,
                    center_chunk_z,
                    radius,
                    Some(CHUNK_SYNC_BATCH_LIMIT),
                    qexed_world::ChunkSyncCause::CompletionTick,
                )
                .await?;
            }
        }
    }
}

#[derive(Clone)]
struct ChatContext {
    profile: GameProfile,
    online_mode: bool,
    authenticator: qexed_auth::Authenticator,
    service: qexed_chat::ChatService,
}

async fn handle_chat_session_update(
    connection: &mut ClientConnection,
    context: &ChatContext,
    chat_session: &mut Option<qexed_chat::SecureChatSession>,
    packet: qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate,
) -> anyhow::Result<()> {
    if context.online_mode {
        context
            .authenticator
            .verify_chat_session(context.profile.uuid, &packet.chat_session)
            .await?;
    }

    *chat_session = Some(qexed_chat::SecureChatSession::new(&packet.chat_session)?);
    connection
        .send_packet(&to_client::play::player_info_update::PlayerInfoUpdate {
            actions: to_client::play::player_info_update::PlayerInfoActions(
                to_client::play::player_info_update::PlayerInfoActions::INITIALIZE_CHAT,
            ),
            entries: vec![to_client::play::player_info_update::PlayerInfoEntry {
                profile_id: context.profile.uuid,
                chat_session: Some(packet.chat_session),
                ..Default::default()
            }],
        })
        .await?;
    connection.flush().await
}

async fn handle_chat_message(
    connection: &mut ClientConnection,
    context: &ChatContext,
    chat_session: &mut Option<qexed_chat::SecureChatSession>,
    next_chat_global_index: &mut i32,
    packet: qexed_protocol::to_server::play::chat_message::ChatMessage,
) -> anyhow::Result<()> {
    let decision = context
        .service
        .process(qexed_chat::ChatMessage {
            sender: context.profile.username.clone(),
            content: packet.message.clone(),
        })
        .await?;

    match decision {
        qexed_chat::ChatResult::Enabled {
            message,
            system_chat_only,
        } => {
            if system_chat_only {
                send_system_chat(
                    connection,
                    text_component(format!(
                        "<{}> {}",
                        context.profile.username, message.content
                    )),
                )
                .await?;
            } else if let Some(chat_session) = chat_session.as_mut() {
                let verified = chat_session.verify_message(context.profile.uuid, &packet)?;
                let packet = to_client::play::player_chat::PlayerChat::pass_through(
                    *next_chat_global_index,
                    context.profile.uuid,
                    verified.index,
                    verified.signature.clone(),
                    verified
                        .last_seen
                        .into_iter()
                        .map(to_client::play::player_chat::PackedMessageSignature::full)
                        .collect(),
                    message.content,
                    packet.timestamp,
                    packet.salt,
                    text_component(context.profile.username.clone()),
                );
                connection.send_packet(&packet).await?;
                chat_session.add_pending_signature(&verified.signature)?;
                *next_chat_global_index = next_chat_global_index
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("chat global index overflow"))?;
            } else if context.online_mode {
                anyhow::bail!("client did not initialize Mojang secure chat session");
            } else {
                connection
                    .send_packet(&to_client::play::player_chat::PlayerChat {
                        global_index: VarInt(0),
                        sender: context.profile.uuid,
                        index: packet.offset,
                        signature: packet.signature,
                        plain_message: message.content,
                        timestamp: packet.timestamp,
                        salt: packet.salt,
                        previous_messages: Vec::new(),
                        unsigned_chat_content: None,
                        filter_type: VarInt(0),
                        chat_type: to_client::play::player_chat::ChatTypeHolder::default(),
                        network_name: text_component(context.profile.username.clone()),
                        network_target_name: None,
                    })
                    .await?;
            }
        }
        qexed_chat::ChatResult::Disabled => {
            send_system_chat(connection, text_component("聊天功能未启用")).await?;
        }
    }

    connection.flush().await
}

async fn handle_chat_command(
    connection: &mut ClientConnection,
    context: &qexed_command::CommandContext,
    command: &str,
) -> anyhow::Result<()> {
    match qexed_command::execute_builtin(command, context) {
        qexed_command::CommandResponse::Message(message) => {
            send_system_chat(connection, text_component(message)).await?;
        }
        qexed_command::CommandResponse::Unknown { command } => {
            send_system_chat(connection, unknown_command_component(&command)).await?;
        }
    }
    connection.flush().await
}

async fn handle_command_suggestion(
    connection: &mut ClientConnection,
    context: &qexed_command::CommandContext,
    suggestion: qexed_protocol::to_server::play::command_suggestion::CommandSuggestion,
) -> anyhow::Result<()> {
    let matches = qexed_command::command_suggestion_matches_with_sources(
        &suggestion.text,
        &context.online_players,
        &[],
    );
    connection
        .send_packet(&to_client::play::command_suggestions::CommandSuggestions {
            id: suggestion.id,
            start: VarInt(matches.start as i32),
            length: VarInt(matches.length as i32),
            matches: matches
                .values
                .into_iter()
                .map(|value| to_client::play::command_suggestions::Matches {
                    r#match: value,
                    tooltip: None,
                })
                .collect(),
        })
        .await?;
    connection.flush().await
}

async fn send_system_chat(
    connection: &mut ClientConnection,
    content: qexed_protocol::types::TextComponent,
) -> anyhow::Result<()> {
    connection
        .send_packet(&to_client::play::system_chat::SystemChat {
            content,
            overlay: false,
        })
        .await
}

async fn send_login_disconnect(
    connection: &mut ClientConnection,
    reason: impl Into<String>,
) -> anyhow::Result<()> {
    connection
        .send_packet(&to_client::login::disconnect::Disconnect {
            reason: JsonValue(serde_json::json!({ "text": reason.into() })),
        })
        .await
}

fn server_session_id() -> uuid::Uuid {
    *SERVER_SESSION_ID.get_or_init(uuid::Uuid::new_v4)
}

async fn read_expected_packet<T, R>(stream: &mut PacketStream<R>) -> anyhow::Result<T>
where
    T: Packet + Default,
    R: AsyncRead + Unpin,
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

async fn with_timeout<T, E, F>(
    duration: Duration,
    operation: &'static str,
    future: F,
) -> anyhow::Result<T>
where
    E: Into<anyhow::Error>,
    F: std::future::Future<Output = Result<T, E>>,
{
    timeout(duration, future)
        .await
        .map_err(|_| anyhow::anyhow!("{operation} timed out after {:?}", duration))?
        .map_err(Into::into)
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

async fn send_packet<T, W>(stream: &mut PacketSink<W>, packet: &T) -> anyhow::Result<()>
where
    T: Packet,
    W: AsyncWrite + Unpin,
{
    stream.send_ref(packet).await?;
    Ok(())
}

#[cfg(test)]
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

async fn read_frame<R>(stream: &mut PacketStream<R>) -> anyhow::Result<BytesMut>
where
    R: AsyncRead + Unpin,
{
    stream
        .read_packet()
        .await?
        .ok_or_else(|| ConnectionClosed.into())
}

#[cfg(test)]
async fn write_frame<W>(stream: &mut PacketSink<W>, payload: &[u8]) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
{
    stream.send_raw(payload).await?;
    Ok(())
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

fn unknown_command_component(command: &str) -> qexed_protocol::types::TextComponent {
    let mut message = std::collections::HashMap::new();
    message.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("command.unknown.command")),
    );
    message.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("red")),
    );

    let mut context_text = std::collections::HashMap::new();
    context_text.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(format!("\n/{command}"))),
    );
    context_text.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("red")),
    );

    let mut context_here = std::collections::HashMap::new();
    context_here.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("command.context.here")),
    );
    context_here.insert(
        "color".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("red")),
    );
    context_here.insert("italic".to_string(), qexed_nbt::Tag::Byte(1));

    message.insert(
        "extra".to_string(),
        qexed_nbt::Tag::List(
            qexed_nbt::ListHeader {
                tag_id: qexed_nbt::tag_id::COMPOUND,
                length: 2,
            },
            std::sync::Arc::from(
                vec![
                    qexed_nbt::Tag::Compound(std::sync::Arc::new(context_text)),
                    qexed_nbt::Tag::Compound(std::sync::Arc::new(context_here)),
                ]
                .into_boxed_slice(),
            ),
        ),
    );

    qexed_nbt::Tag::Compound(std::sync::Arc::new(message))
}

#[cfg(test)]
mod tests {
    use super::{
        ConnectionIo, build_payload, chunk_source_summary, decode_payload, first_loaded_chunk,
        first_unloaded_chunk, read_frame, read_packet_id, write_frame,
    };
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
    use std::path::{Path, PathBuf};
    use tokio::net::{TcpListener, TcpStream};

    #[test]
    fn packet_payload_starts_with_packet_id() {
        let payload = build_payload(1, &Ping { time: 42 }).unwrap();

        assert_eq!(payload[0], 1);
    }

    #[test]
    fn varint_writer_uses_minecraft_encoding() {
        let payload = build_payload(300, &Ping { time: 42 }).unwrap();

        assert_eq!(&payload[..2], &[0xac, 0x02]);
    }

    #[test]
    fn offline_profile_uses_client_uuid_when_present() {
        let uuid = uuid::Uuid::from_u128(1);
        let profile = qexed_auth::offline_profile_with_client_uuid("Steve", uuid);

        assert_eq!(profile.uuid, uuid);
    }

    #[test]
    fn chunk_source_summary_reports_structured_load_sources() {
        let event = qexed_world::ChunkSyncEvent::new(
            qexed_world::ChunkSyncCause::PlayerMove,
            2,
            0,
            vec![
                qexed_world::ChunkLoadEvent::new(2, 0, qexed_world::NetworkChunkLoadSource::Saved),
                qexed_world::ChunkLoadEvent::new(
                    3,
                    0,
                    qexed_world::NetworkChunkLoadSource::EmptyFallback,
                ),
            ],
            vec![qexed_world::ChunkUnloadEvent::new(0, 0)],
        );

        assert_eq!(first_loaded_chunk(&event), "(2, 0)");
        assert_eq!(first_unloaded_chunk(&event), "(0, 0)");
        assert_eq!(
            chunk_source_summary(&event),
            "saved=1, local_generated=0, vanilla_generated=0, empty_fallback=1"
        );
    }

    #[test]
    fn moved_chunk_window_identifies_chunks_to_unload() {
        let old_window = super::chunk_window_positions(0, 0, 1);
        let new_window = super::chunk_window_positions(2, 0, 1);
        let mut leaving_chunks = old_window
            .difference(&new_window)
            .copied()
            .collect::<Vec<_>>();
        leaving_chunks.sort();

        assert_eq!(
            leaving_chunks,
            vec![(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 0), (0, 1)]
        );
    }

    #[tokio::test]
    async fn with_timeout_reports_slow_operation() {
        let err = super::with_timeout(
            std::time::Duration::from_millis(1),
            "test operation",
            async {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Ok::<_, anyhow::Error>(())
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("test operation timed out"));
    }

    #[tokio::test]
    async fn status_handshake_responds_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        write_frame(
            &mut client.writer,
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
            &mut client.writer,
            &build_payload(PingStart::ID, &PingStart {}).unwrap(),
        )
        .await
        .unwrap();

        let mut payload = read_frame(&mut client.reader).await.unwrap();
        assert_eq!(read_packet_id(&mut payload).unwrap(), ServerInfo::ID);
        let response = decode_payload::<ServerInfo>(&mut payload).unwrap();

        assert_eq!(
            response.response.0["version"]["protocol"],
            qexed_config::PROTOCOL_VERSION
        );
        assert_eq!(response.response.0["enforcesSecureChat"], false);
        drop(client);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn online_status_reports_secure_chat_enforced() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = online_runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        write_frame(
            &mut client.writer,
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
            &mut client.writer,
            &build_payload(PingStart::ID, &PingStart {}).unwrap(),
        )
        .await
        .unwrap();

        let mut payload = read_frame(&mut client.reader).await.unwrap();
        assert_eq!(read_packet_id(&mut payload).unwrap(), ServerInfo::ID);
        let response = decode_payload::<ServerInfo>(&mut payload).unwrap();

        assert_eq!(response.response.0["enforcesSecureChat"], true);
        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn login_reaches_play_and_sends_initial_chunks() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
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
        let mut saw_commands = false;
        let mut saw_batch_start = false;
        let mut map_chunks = 0;
        let mut saw_batch_finished = false;

        for _ in 0..1024 {
            let packet_id = read_next_packet_id(&mut client).await;
            match packet_id {
                to_client::play::login::Login::ID => saw_login = true,
                to_client::play::position::Position::ID => saw_position = true,
                to_client::play::player_info_update::PlayerInfoUpdate::ID => saw_player_info = true,
                to_client::play::game_state_change::GameStateChange::ID => saw_load_start = true,
                to_client::play::commands::Commands::ID => saw_commands = true,
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
        assert!(saw_commands, "command tree packet was not sent");
        assert!(saw_batch_start, "chunk batch start packet was not sent");
        assert!(map_chunks >= 1, "initial map chunk was not sent");
        assert!(
            saw_batch_finished,
            "chunk batch finished packet was not sent"
        );

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn login_generates_initial_chunks_when_worldgen_is_available() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = runtime_config();
        let temp = tempfile::tempdir().unwrap();
        let mut save_config = qexed_config::app::qexed_save::Save::default();
        save_config.root.universe = temp.path().to_string_lossy().to_string();
        let save = qexed_save::SaveService::new(save_config).unwrap();
        config.save = save.clone();
        config.world = qexed_world::WorldManager::new(save);
        config.local_worldgen = Some(std::sync::Arc::new(
            qexed_worldgen::WorldGenerator::default_cache(0).expect("local worldgen should start"),
        ));

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        assert!(
            temp.path()
                .join("world/dimensions/minecraft/overworld/region/r.0.0.mca")
                .exists()
        );

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_worldgen_can_be_compared_with_java_oracle_when_enabled() {
        if std::env::var_os("QEXED_WORLDGEN_COMPARE_JAVA").is_none() {
            return;
        }
        init_registry_for_connection_test();

        let temp = tempfile::tempdir().unwrap();
        let mut save_config = qexed_config::app::qexed_save::Save::default();
        save_config.root.universe = temp.path().to_string_lossy().to_string();
        let save = qexed_save::SaveService::new(save_config).unwrap();
        save.initialize_directories().unwrap();
        let manager = qexed_world::WorldManager::new(save);
        let dimension = qexed_save::DimensionId::overworld();
        ensure_java_oracle_region_cached(&manager, &dimension, 0, 0, 0).await;

        let generator =
            qexed_worldgen::WorldGenerator::default_cache(0).expect("local worldgen should start");
        let comparison = manager
            .compare_local_worldgen_with_region_chunk(&generator, &dimension, 0, 0)
            .expect("worldgen comparison should run");

        assert!(
            comparison.equal,
            "local worldgen differs from Java oracle: {}",
            comparison.differences.join(" | ")
        );
    }

    async fn ensure_java_oracle_region_cached(
        manager: &qexed_world::WorldManager,
        dimension: &qexed_save::DimensionId,
        seed: i64,
        chunk_x: i32,
        chunk_z: i32,
    ) {
        let target_region = manager.storage().region_path(dimension, chunk_x, chunk_z);
        let cache_region = java_oracle_region_cache_path(dimension, seed, chunk_x, chunk_z);
        if cache_region.exists() {
            copy_region_file(&cache_region, &target_region)
                .expect("cached Java oracle region should copy into test save");
            return;
        }

        let process = crate::worldgen_process::WorldgenProcess::spawn()
            .expect("Java worldgen oracle should start");
        let client =
            qexed_world::generator_rpc::VanillaWorldgenClient::new(process.endpoint().to_string());
        manager
            .request_vanilla_generated_chunk(&client, dimension, chunk_x, chunk_z)
            .await
            .expect("Java oracle should generate chunk");
        copy_region_file(&target_region, &cache_region)
            .expect("Java oracle region should be saved into cache");
    }

    fn java_oracle_region_cache_path(
        dimension: &qexed_save::DimensionId,
        seed: i64,
        chunk_x: i32,
        chunk_z: i32,
    ) -> PathBuf {
        let region_x = chunk_x.div_euclid(32);
        let region_z = chunk_z.div_euclid(32);
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root should resolve")
            .join("target")
            .join("worldgen-oracle")
            .join("26.2")
            .join(format!("seed-{seed}"))
            .join(dimension.namespace())
            .join(dimension.value())
            .join("region")
            .join(format!("r.{region_x}.{region_z}.mca"))
    }

    fn copy_region_file(from: &Path, to: &Path) -> std::io::Result<()> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to)?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn moving_between_chunks_syncs_chunk_window() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = runtime_config();
        config.qexed.server.view_distance = 1;

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::move_player_pos::MovePlayerPos {
                x: 32.5,
                y: 64.0,
                z: 0.5,
                flags: 0,
            },
        )
        .await;

        let mut saw_view_position = false;
        let mut map_chunks = 0;
        let mut saw_unload = false;
        let mut batch_size = None;
        for _ in 0..1024 {
            let mut payload = read_frame(&mut client.reader).await.unwrap();
            match read_packet_id(&mut payload).unwrap() {
                to_client::play::update_view_position::UpdateViewPosition::ID => {
                    let packet = decode_payload::<
                        to_client::play::update_view_position::UpdateViewPosition,
                    >(&mut payload)
                    .unwrap();
                    if packet.chunk_x.0 == 2 && packet.chunk_z.0 == 0 {
                        saw_view_position = true;
                    }
                }
                to_client::play::chunk_batch_start::ChunkBatchStart::ID => map_chunks = 0,
                to_client::play::map_chunk::MapChunk::ID => map_chunks += 1,
                to_client::play::forget_level_chunk::ForgetLevelChunk::ID => saw_unload = true,
                to_client::play::chunk_batch_finished::ChunkBatchFinished::ID => {
                    let packet = decode_payload::<
                        to_client::play::chunk_batch_finished::ChunkBatchFinished,
                    >(&mut payload)
                    .unwrap();
                    batch_size = Some(packet.batch_size.0);
                }
                _ => {}
            }
            if batch_size.is_some() && saw_unload {
                break;
            }
        }

        assert!(
            saw_view_position,
            "view center was not updated after movement"
        );
        assert!(map_chunks > 0);
        assert_eq!(batch_size, Some(map_chunks));
        assert!(saw_unload, "old chunks were not unloaded after movement");

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_chat_command_returns_system_chat() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::chat_command::ChatCommand {
                command: "help".to_string(),
            },
        )
        .await;

        wait_for_play_packet(&mut client, to_client::play::system_chat::SystemChat::ID).await;

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_chat_message_returns_player_chat_when_enabled() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::chat_message::ChatMessage {
                message: "你好".to_string(),
                timestamp: 0,
                salt: 0,
                signature: None,
                offset: VarInt(0),
                acknowledged: [0; 3],
                checksum: 0,
            },
        )
        .await;

        wait_for_play_packet(&mut client, to_client::play::player_chat::PlayerChat::ID).await;

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_chat_message_returns_system_chat_when_disabled() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = runtime_config();
        config.chat = disabled_chat_service();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::chat_message::ChatMessage {
                message: "hello".to_string(),
                timestamp: 0,
                salt: 0,
                signature: None,
                offset: VarInt(0),
                acknowledged: [0; 3],
                checksum: 0,
            },
        )
        .await;

        wait_for_play_packet(&mut client, to_client::play::system_chat::SystemChat::ID).await;

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_chat_message_returns_system_chat_when_forced() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = runtime_config();
        config.chat = system_chat_only_service();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::chat_message::ChatMessage {
                message: "hello".to_string(),
                timestamp: 0,
                salt: 0,
                signature: None,
                offset: VarInt(0),
                acknowledged: [0; 3],
                checksum: 0,
            },
        )
        .await;

        wait_for_play_packet(&mut client, to_client::play::system_chat::SystemChat::ID).await;

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn online_mode_chat_message_without_session_closes_connection() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = runtime_config();
        config.authenticator =
            qexed_auth::Authenticator::new(qexed_config::app::qexed_auth::Auth {
                enabled: true,
                yggdrasil: Default::default(),
                blocking_pool: Default::default(),
            });

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let stream = stream.into_split();
            let mut connection = ConnectionIo::new(stream.0, stream.1);

            let session = super::LoginSession {
                profile: qexed_auth::offline_profile_with_client_uuid(
                    "Steve",
                    uuid::Uuid::from_u128(1),
                ),
                online_mode: true,
            };
            if let Err(err) = super::enter_play(
                &mut connection,
                &config,
                config.authenticator.clone(),
                config.chat.clone(),
                &session,
            )
            .await
            {
                assert!(
                    err.to_string().contains("secure chat session")
                        || super::is_expected_disconnect(&err),
                    "{err:#}"
                );
            }
        });

        let mut client = connect(addr).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::chat_message::ChatMessage {
                message: "hello".to_string(),
                timestamp: 0,
                salt: 0,
                signature: None,
                offset: VarInt(0),
                acknowledged: [0; 3],
                checksum: 0,
            },
        )
        .await;

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn chat_session_update_initializes_player_chat() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate {
                chat_session: qexed_protocol::types::ChatSessionData {
                    session_id: uuid::Uuid::from_u128(2),
                    expires_at_epoch_millis: i64::MAX,
                    public_key_der: test_public_key_der(),
                    key_signature: vec![1, 2, 3],
                },
            },
        )
        .await;

        let packet = wait_for_play_packet_decoded::<
            _,
            _,
            to_client::play::player_info_update::PlayerInfoUpdate,
        >(&mut client)
        .await;

        assert_eq!(
            packet.actions.0,
            to_client::play::player_info_update::PlayerInfoActions::INITIALIZE_CHAT
        );
        assert!(packet.entries[0].chat_session.is_some());

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn online_play_login_enforces_secure_chat() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = online_runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let stream = stream.into_split();
            let mut connection = ConnectionIo::new(stream.0, stream.1);

            let session = super::LoginSession {
                profile: qexed_auth::offline_profile_with_client_uuid(
                    "Steve",
                    uuid::Uuid::from_u128(1),
                ),
                online_mode: true,
            };
            if let Err(err) = super::enter_play(
                &mut connection,
                &config,
                config.authenticator.clone(),
                config.chat.clone(),
                &session,
            )
            .await
            {
                assert!(super::is_expected_disconnect(&err), "{err:#}");
            }
        });

        let mut client = connect(addr).await;
        let login =
            wait_for_play_packet_decoded::<_, _, to_client::play::login::Login>(&mut client).await;

        assert!(login.online_mode);
        assert!(login.enforces_secure_chat);

        drop(client);
        server.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_command_suggestion_returns_matches() {
        init_registry_for_connection_test();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = runtime_config();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle(stream, config).await;
        });

        let mut client = connect(addr).await;
        login_to_play(&mut client).await;
        wait_for_play_packet(
            &mut client,
            to_client::play::chunk_batch_finished::ChunkBatchFinished::ID,
        )
        .await;

        write_packet(
            &mut client,
            &qexed_protocol::to_server::play::command_suggestion::CommandSuggestion {
                id: VarInt(42),
                text: "/help v".to_string(),
            },
        )
        .await;

        let response = wait_for_play_packet_decoded::<
            _,
            _,
            to_client::play::command_suggestions::CommandSuggestions,
        >(&mut client)
        .await;
        assert_eq!(response.id.0, 42);
        assert_eq!(response.start.0, 6);
        assert_eq!(response.length.0, 1);
        assert_eq!(response.matches.len(), 1);
        assert_eq!(response.matches[0].r#match, "version");

        drop(client);
        server.await.unwrap();
    }

    async fn connect(
        addr: std::net::SocketAddr,
    ) -> ConnectionIo<tokio::net::tcp::OwnedReadHalf, tokio::net::tcp::OwnedWriteHalf> {
        let stream = TcpStream::connect(addr).await.unwrap();
        let (reader, writer) = stream.into_split();
        ConnectionIo::new(reader, writer)
    }

    async fn write_packet<T, R, W>(connection: &mut ConnectionIo<R, W>, packet: &T)
    where
        T: Packet,
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        write_frame(
            &mut connection.writer,
            &build_payload(T::ID, packet).unwrap(),
        )
        .await
        .unwrap();
    }

    async fn read_next_packet_id<R, W>(connection: &mut ConnectionIo<R, W>) -> i32
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut payload = read_frame(&mut connection.reader).await.unwrap();
        read_packet_id(&mut payload).unwrap()
    }

    async fn login_to_play<R, W>(connection: &mut ConnectionIo<R, W>)
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        write_packet(
            connection,
            &SetProtocol {
                protocol_version: VarInt(qexed_config::PROTOCOL_VERSION),
                server_host: "localhost".to_string(),
                server_port: 25565,
                next_state: VarInt(2),
            },
        )
        .await;
        write_packet(
            connection,
            &LoginStart {
                username: "Steve".to_string(),
                player_uuid: uuid::Uuid::from_u128(1),
            },
        )
        .await;

        assert_eq!(
            read_next_packet_id(connection).await,
            to_client::login::success::Success::ID
        );
        write_packet(connection, &LoginAcknowledged {}).await;
        wait_for_clientbound_config_select_known_packs(connection).await;
        write_packet(
            connection,
            &ServerboundSelectKnownPacks {
                entries: qexed_registry::known_packs(),
            },
        )
        .await;
        wait_for_clientbound_finish_configuration(connection).await;
        write_packet(connection, &ServerboundFinishConfiguration {}).await;
    }

    async fn wait_for_play_packet<R, W>(connection: &mut ConnectionIo<R, W>, expected_id: i32)
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        for _ in 0..1024 {
            if read_next_packet_id(connection).await == expected_id {
                return;
            }
        }
        panic!("did not receive play packet {expected_id}");
    }

    async fn wait_for_play_packet_decoded<R, W, T>(connection: &mut ConnectionIo<R, W>) -> T
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
        T: Packet + Default,
    {
        for _ in 0..1024 {
            let mut payload = read_frame(&mut connection.reader).await.unwrap();
            if read_packet_id(&mut payload).unwrap() == T::ID {
                return decode_payload::<T>(&mut payload).unwrap();
            }
        }
        panic!("did not receive play packet {}", T::ID);
    }

    async fn wait_for_clientbound_config_select_known_packs<R, W>(
        connection: &mut ConnectionIo<R, W>,
    ) where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        loop {
            let packet_id = read_next_packet_id(connection).await;
            if packet_id == to_client::configuration::select_known_packs::SelectKnownPacks::ID {
                return;
            }
        }
    }

    async fn wait_for_clientbound_finish_configuration<R, W>(connection: &mut ConnectionIo<R, W>)
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        loop {
            let packet_id = read_next_packet_id(connection).await;
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

    fn runtime_config() -> crate::bootstrap::RuntimeConfig {
        let mut save_config = qexed_config::app::qexed_save::Save::default();
        save_config.initialize_directories = false;
        let save = qexed_save::SaveService::new(save_config).unwrap();
        crate::bootstrap::RuntimeConfig {
            qexed: qexed_config::app::qexed::Qexed::default(),
            authenticator: qexed_auth::Authenticator::new(Default::default()),
            chat: qexed_chat::ChatService::new(Default::default()).unwrap(),
            save: save.clone(),
            world: qexed_world::WorldManager::new(save),
            local_worldgen: None,
        }
    }

    fn online_runtime_config() -> crate::bootstrap::RuntimeConfig {
        let mut config = runtime_config();
        config.authenticator =
            qexed_auth::Authenticator::new(qexed_config::app::qexed_auth::Auth {
                enabled: true,
                yggdrasil: Default::default(),
                blocking_pool: Default::default(),
            });
        config
    }

    fn disabled_chat_service() -> qexed_chat::ChatService {
        let mut config = qexed_config::app::qexed_chat::Chat::default();
        config.enabled = false;
        qexed_chat::ChatService::new(config).unwrap()
    }

    fn system_chat_only_service() -> qexed_chat::ChatService {
        let mut config = qexed_config::app::qexed_chat::Chat::default();
        config.system_chat_only = true;
        qexed_chat::ChatService::new(config).unwrap()
    }

    fn test_public_key_der() -> Vec<u8> {
        qexed_auth::Authenticator::new(Default::default())
            .public_key_der()
            .unwrap()
    }
}
