//! play 会话核心（v4 play.rs 的 initialize/wait_for_play_packets 迁移）。
//!
//! v6 适配总览：
//! - PacketSink/PacketStream 改用 qexed_connection::transport（v4 qexed_tcp_connect）
//! - Position（登录同步坐标）-> PlayerPosition + PositionMoveRotation
//! - GameStateChange -> GameEvent；move_player_pos/pos_rot -> pos/pos_rot（i8 角度）
//! - WorldManager/WorldRulesManager -> [`WorldChunkSource`]/[`WorldRulesSource`] trait
//! - 生存/挖矿/玩法/聊天命令/计分板/Geyser 子系统归 play-gameplay 任务，本核心经
//!   [`GameplayHooks`] trait 回调（默认 NoGameplay 为 no-op）；
//!   TODO(play-gameplay)：各子系统迁移后替换为直接调用
//! - 集群实体视图经 [`ClusterEntityView`] trait（None 时跳过）
//! - SecureChat/Authenticator 经 [`SecureChatHook`] trait
//! - 内容过滤经 [`ChatFilter`] trait（v4 content_filter；server 域有同名配置）

// play-gameplay 接线前部分 API 暂未被本 crate 引用（供其改造 runtime.rs 时使用）。
#![allow(dead_code)]

use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use qexed_connection::transport::{PacketSink, PacketStream};
use qexed_packet::{
    Packet, PacketCodec,
    net_types::{Position as BlockPosition, VarInt, VarLong},
};
use qexed_player::{
    OnlinePlayer, PlayerData, PlayerDataLockGuard, PlayerDataManager, PlayerEvent,
    PlayerManager, PlayerSession,
};
use qexed_protocol::to_client::play::{
    game_event::GameEvent,
    keep_alive::KeepAlive as ClientboundKeepAlive,
    player_abilities::PlayerAbilities as ClientboundPlayerAbilities,
    player_info_update::{PlayerInfoActions, PlayerInfoEntry, PlayerInfoUpdate},
    player_position::{PositionMoveRotation, PlayerPosition},
    remove_entities::RemoveEntities,
    system_chat::SystemChat,
};
use qexed_protocol::to_server::play::{
    accept_teleportation::AcceptTeleportation,
    chat_ack::ChatAck,
    chat_command::ChatCommand,
    chunk_batch_received::ChunkBatchReceived,
    keep_alive::KeepAlive as ServerboundKeepAlive,
    player_loaded::PlayerLoaded,
    pos::Pos,
    pos_rot::PosRot,
    rot::Rot,
    status_only::StatusOnly,
};
use qexed_protocol::types::EntityPosition;

use crate::bootstrap::{self, ServerDisplay, SessionInventory, SurvivalSnapshot};
use crate::chunks::{self, ChunkSendState, SharedWorld};
use crate::config::{GameMode, PlayConfig};
use crate::context::{
    ClusterEntityView, PermissionLookup, PlayerAuditLog, PluginEventSink, WorldRulesSource,
};
use crate::drops::ItemRegistry;
use crate::error::{PlayError, Result};
use crate::events::{event_is_self, player_event_message};
use crate::lobby::LobbyRuntime;
use crate::menus::MenuRuntime;
use crate::util::{
    chunk_coord, dimension_type_holder_id, keep_alive_id, player_ability_flags, spawn_position,
    text_component,
};

const KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
const CHUNK_SEND_TICK_INTERVAL: Duration = Duration::from_millis(50);
const CHUNK_UNLOAD_SWEEP_INTERVAL: Duration = Duration::from_millis(500);
const GAMEPLAY_TICK_INTERVAL: Duration = Duration::from_millis(50);
const SURVIVAL_TICK_INTERVAL: Duration = Duration::from_secs(1);
const WORLD_TIME_TICK_INTERVAL: Duration = Duration::from_secs(1);
const MIN_PLAYER_DATA_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(10);
const PLAYER_ACTION_DROP_ITEM_STACK: i32 = 3;
const PLAYER_ACTION_DROP_ITEM: i32 = 4;
const MOVE_FLAG_ON_GROUND: u8 = 0x01;

// ── Fluid system（v4 已移除，保留空运行时以兼容会话流程） ──

pub struct FluidRuntime;

impl FluidRuntime {
    pub fn new() -> Self {
        Self
    }

    pub fn enqueue_fluid_seeds(
        &self,
        _: &str,
        _: impl IntoIterator<Item = crate::chunks::FluidSeed>,
    ) {
    }
}

impl Default for FluidRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ── 聊天限速（v4 ChatRateLimit） ──

#[derive(Debug)]
pub struct ChatRateLimit {
    limit: u32,
    window: Duration,
    events: std::collections::VecDeque<std::time::Instant>,
}

impl ChatRateLimit {
    pub fn new(limit: u32, window_secs: u64) -> Self {
        Self {
            limit,
            window: Duration::from_secs(window_secs.max(1)),
            events: std::collections::VecDeque::new(),
        }
    }

    fn allow(&mut self, now: std::time::Instant) -> bool {
        while let Some(front) = self.events.front() {
            if now.duration_since(*front) > self.window {
                self.events.pop_front();
            } else {
                break;
            }
        }
        if self.events.len() >= self.limit as usize {
            return false;
        }
        self.events.push_back(now);
        true
    }
}

fn validate_player_chat_message(rate_limit: &mut ChatRateLimit, message: &str) -> Option<String> {
    let now = std::time::Instant::now();
    if message.trim().is_empty() {
        return Some(qexed_language::t("qexed.play.chat.empty"));
    }
    if !rate_limit.allow(now) {
        return Some(qexed_language::t("qexed.play.chat.rate_limited"));
    }
    None
}

// ── gameplay/play-gameplay 边界 hook ──

/// 每回合传给 gameplay 侧的会话快照（位置/维度/背包句柄等最小面）。
pub struct SessionTickContext<'a> {
    pub profile_id: uuid::Uuid,
    pub entity_id: i32,
    pub dimension: &'a str,
    pub position: EntityPosition,
    pub game_mode: GameMode,
}

/// gameplay 域回调面（v4 play.rs 内联调用的 survival/gameplay/mining/chat 等子系统的收敛点）。
///
/// TODO(play-gameplay)：survival / gameplay / mining / chat / scoreboard / geyser /
/// recipes 模块迁移后，装配层实现本 trait 并接通完整逻辑；当前 [`NoGameplay`]
/// 提供最小 no-op 行为（会话循环照常运转，仅无 gameplay 效果）。
pub trait GameplayHooks: Send + Sync {
    /// 生存移动副作用（坠落伤害等）。返回是否发生传送。
    fn survival_movement(
        &self,
        _ctx: &SessionTickContext<'_>,
        _previous: EntityPosition,
    ) -> Result<bool> {
        Ok(false)
    }

    /// 生存 tick（饥饿/回复）。返回死亡消息。
    fn survival_tick(&self, _ctx: &SessionTickContext<'_>) -> Result<Option<String>> {
        Ok(None)
    }

    /// 应用外部伤害（其他玩家/实体对会话玩家）。返回是否死亡及消息。
    fn apply_damage(
        &self,
        _ctx: &SessionTickContext<'_>,
        _amount: f32,
        _kind: qexed_player::PlayerDamageKind,
        _source_entity_id: i32,
        _source_position: EntityPosition,
        _knockback: f32,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    /// 处理玩家死亡（重生流程由 gameplay 域接管）。返回重生位置。
    fn handle_death(&self, _ctx: &SessionTickContext<'_>, _message: &str) -> Result<()> {
        Ok(())
    }

    /// gameplay tick（红石/流体/环境/熔炉等方块世界 tick，v4 gameplay_tick 分支）。
    fn gameplay_tick(&self, _ctx: &SessionTickContext<'_>) -> Result<()> {
        Ok(())
    }

    /// 插件 player tick（v4 plugins.handle_player_tick + response actions）。
    fn plugin_player_tick(&self, _ctx: &SessionTickContext<'_>, _tick_ms: u64) -> Result<()> {
        Ok(())
    }

    /// 命令处理（v4 chat::handle_chat_command）。返回是否传送。
    fn handle_command(&self, _ctx: &SessionTickContext<'_>, _command: &str) -> Result<bool> {
        Ok(false)
    }

    /// 侧边栏刷新包（v4 scoreboard::refresh_lobby_sidebar_packets）。
    fn sidebar_packets(&self) -> Result<Vec<bytes::Bytes>> {
        Ok(Vec::new())
    }

    /// 世界写冲刷（v4 world.flush_block_writes；autosave 后调用）。
    fn flush_world_writes(&self) {}

    /// 行走方块踩踏/交互（v4 handle_plugin_player_block_step / block_interact）。
    fn player_block_step(&self, _ctx: &SessionTickContext<'_>) -> Result<bool> {
        Ok(false)
    }

    /// 玩家移动插件通知（v4 handle_plugin_player_move）。
    fn player_move(&self, _ctx: &SessionTickContext<'_>, _previous: EntityPosition) -> Result<bool> {
        Ok(false)
    }

    /// 玩家输入插件通知（v4 handle_plugin_player_input）。
    fn player_input(&self, _ctx: &SessionTickContext<'_>, _previous_flags: u8, _flags: u8) -> Result<bool> {
        Ok(false)
    }

    /// 附近掉落物拾取（v4 collect_nearby_drops）。
    fn collect_nearby_drops(&self, _ctx: &SessionTickContext<'_>) -> Result<()> {
        Ok(())
    }

    /// 背包变更持久化快照（v4 save_player_runtime 的 inventory 部分）。
    fn inventory_snapshot(&self) -> qexed_player::StoredInventory {
        qexed_player::StoredInventory::default()
    }
}

/// 空 gameplay 实现（no-op）。
#[derive(Debug, Default)]
pub struct NoGameplay;

impl GameplayHooks for NoGameplay {}

// ── 安全聊天 hook ──

/// 安全聊天/认证面（v4 secure_chat::SecureChatSession + authenticator.verify_chat_session）。
///
/// TODO(connection)：qexed_connection::secure_chat 已迁移，装配层实现本 trait 接入。
pub trait SecureChatHook: Send + Sync {
    /// 校验聊天会话更新；返回是否接受。
    fn verify_session_update(
        &self,
        _session: &qexed_protocol::types::ChatSessionData,
    ) -> Result<()> {
        Ok(())
    }

    /// 校验消息签名；返回通过后的广播包（None 表示本地不校验签名）。
    fn verify_message(
        &self,
        _message: &qexed_protocol::to_server::play::chat::Chat,
    ) -> Result<Option<bytes::Bytes>> {
        Ok(None)
    }

    /// 应用 chat ack 偏移。
    fn apply_ack(&self, _offset: VarInt) -> Result<()> {
        Ok(())
    }
}

/// 不校验实现（离线模式）。
#[derive(Debug, Default)]
pub struct NoSecureChat;

impl SecureChatHook for NoSecureChat {}

// ── 聊天过滤 ──

/// 聊天内容过滤（v4 content_filter.check_chat）。
pub trait ChatFilter: Send + Sync {
    /// Allow(过滤后消息) / Block{reason}。
    fn check_chat(&self, message: &str) -> Result<FilterAction>;
}

/// 过滤结果（v4 FilterAction）。
pub enum FilterAction {
    Allow(String),
    Block { reason: String },
}

/// 无过滤实现。
#[derive(Debug, Default)]
pub struct NoChatFilter;

impl ChatFilter for NoChatFilter {
    fn check_chat(&self, message: &str) -> Result<FilterAction> {
        Ok(FilterAction::Allow(message.to_string()))
    }
}

// ── 会话初始化 ──

/// 会话依赖集合（v4 initialize 的参数收敛；由装配层构造）。
pub struct PlaySessionDeps<'a> {
    pub config: &'a PlayConfig,
    pub world: SharedWorld,
    pub world_rules: &'a dyn WorldRulesSource,
    pub players: &'a PlayerManager,
    pub player_data: &'a PlayerDataManager,
    pub entities: &'a qexed_entities::EntityManager,
    pub plugins: &'a dyn PluginEventSink,
    pub permissions: &'a dyn PermissionLookup,
    pub player_audit: &'a dyn PlayerAuditLog,
    pub items: &'a dyn ItemRegistry,
    pub inventory: &'a dyn SessionInventory,
    pub gameplay: &'a dyn GameplayHooks,
    pub secure_chat: &'a dyn SecureChatHook,
    pub chat_filter: &'a dyn ChatFilter,
    pub cluster: Option<&'a dyn ClusterEntityView>,
    pub display: ServerDisplay,
    pub command_tree: bytes::Bytes,
    pub entity_rendering: qexed_entities::EntityRendering,
    pub player_entity_type: i32,
}

/// 初始化 play 会话并发包初始状态（v4 play::initialize）。
pub async fn initialize<R, W>(
    packets: &mut PacketStream<R>,
    sink: &mut PacketSink<W>,
    deps: &PlaySessionDeps<'_>,
    profile: &qexed_packet::net_types::GameProfile,
    client_language: Option<String>,
    displayed_skin_parts: u8,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let world_config = &deps.config.world;
    let default_dimension = world_config.default_dimension().to_string();
    let player_data_lock = deps
        .player_data
        .lock_player(profile.uuid)
        .await
        .map_err(|err| {
            PlayError::msg(format!(
                "acquire player data lock for {} ({}): {err}",
                profile.username, profile.uuid
            ))
        })?;
    let spawn = qexed_player::player_data::Spawn {
        x: world_config.spawn.x,
        y: world_config.spawn.y,
        z: world_config.spawn.z,
        yaw: world_config.spawn.yaw,
        pitch: world_config.spawn.pitch,
    };
    let mut saved_player = deps
        .player_data
        .locked_load_or_default(&player_data_lock, profile, &default_dimension, &spawn)
        .await;
    let play_dimension = if saved_player.dimension.is_empty() {
        default_dimension
    } else {
        saved_player.dimension.clone()
    };
    deps.world_rules.ensure_loaded(&play_dimension)?;
    let play_rule = deps.world_rules.snapshot(&play_dimension);
    let view_distance = world_config.view_distance.max(1);
    let chunk_load_parallelism =
        chunks::chunk_load_parallelism_limit(world_config.chunk_load_parallelism);
    let simulation_distance = world_config.simulation_distance.max(1);
    let stored_was_dead = saved_player.survival.health <= 0.0;
    // v4 从存档恢复生存态并按死亡标记决定出生点；v6 生存态由 gameplay 域维护，
    // 这里仅按死亡标记回出生点。
    let player_position = if stored_was_dead {
        spawn_position(&world_config.spawn)
    } else {
        saved_player.entity_position()
    };
    let spawn_chunk_x = chunk_coord(player_position.x);
    let spawn_chunk_z = chunk_coord(player_position.z);
    let language = client_language.unwrap_or_else(|| "zh-CN".to_string());
    let session = deps.players.join_with_skin_parts(
        profile.clone(),
        player_position,
        play_dimension.clone(),
        deps.inventory.visible_equipment(),
        language,
        displayed_skin_parts,
        world_config.game_mode.protocol_id(),
    );
    let _world_session = crate::context::WorldSessionGuard::new(deps.world.as_ref());

    log::debug!(
        "initializing Play state: dimension={}, spawn=({}, {}, {}), yaw={}, pitch={}",
        play_dimension,
        player_position.x,
        player_position.y,
        player_position.z,
        player_position.yaw,
        player_position.pitch
    );

    sink.send(qexed_protocol::to_client::play::login::Login {
        player_id: session.player.entity_id,
        hardcore: false,
        levels: vec![play_dimension.clone()],
        max_players: VarInt(20),
        chunk_radius: VarInt(view_distance),
        simulation_distance: VarInt(simulation_distance),
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        common_player_spawn_info:
            qexed_protocol::to_client::play::login::CommonPlayerSpawnInfo {
                dimension_type: VarInt(dimension_type_holder_id(&play_rule.dimension_type)),
                dimension: play_dimension.clone(),
                seed: 0,
                game_mode: VarInt(world_config.game_mode.protocol_id()),
                previous_game_mode: qexed_packet::net_types::OptionalVarInt(None),
                is_debug: false,
                is_flat: true,
                last_death_location: None,
                portal_cooldown: VarInt(0),
                sea_level: VarInt(63),
            },
        online_mode: false,
        enforces_secure_chat: false,
    })
    .await?;

    let mut next_teleport_id = 1;
    sink.send(PlayerPosition {
        id: VarInt(next_teleport_id),
        change: PositionMoveRotation {
            x: player_position.x,
            y: player_position.y,
            z: player_position.z,
            delta_x: 0.0,
            delta_y: 0.0,
            delta_z: 0.0,
            y_rot: player_position.yaw,
            x_rot: player_position.pitch,
        },
        relatives: 0,
    })
    .await?;
    next_teleport_id += 1;

    bootstrap::send_initial_player_state(
        sink,
        world_config,
        deps.world_rules,
        &deps.display,
        &play_dimension,
        &session.player,
        deps.inventory,
        SurvivalSnapshot::from_stored(
            saved_player.survival.health,
            saved_player.survival.food,
            saved_player.survival.saturation,
        ),
        deps.command_tree.clone(),
    )
    .await?;
    bootstrap::send_existing_players(
        sink,
        deps.players,
        profile.uuid,
        deps.player_entity_type,
        &play_dimension,
        player_position,
        deps.entity_rendering.player_distance,
    )
    .await?;
    bootstrap::send_existing_entities(
        sink,
        deps.entities,
        &play_dimension,
        player_position,
        &deps.entity_rendering,
    )
    .await?;

    let mut chunk_state = ChunkSendState::new(
        play_dimension.clone(),
        spawn_chunk_x,
        spawn_chunk_z,
        view_distance,
        chunk_load_parallelism,
    );
    chunk_state.set_center_update_delay(Duration::from_millis(world_config.chunk_update_delay_ms));
    sink.flush().await?;

    let result = wait_for_play_packets(
        packets,
        sink,
        deps,
        session,
        profile,
        &mut saved_player,
        &player_data_lock,
        play_dimension,
        chunk_state,
        next_teleport_id,
        shutdown,
    )
    .await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn wait_for_play_packets<R, W>(
    packets: &mut PacketStream<R>,
    sink: &mut PacketSink<W>,
    deps: &PlaySessionDeps<'_>,
    mut session: PlayerSession,
    profile: &qexed_packet::net_types::GameProfile,
    saved_player: &mut PlayerData,
    _player_data_lock: &PlayerDataLockGuard,
    mut play_dimension: String,
    mut chunk_state: ChunkSendState,
    _next_teleport_id: i32,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let world = deps.world.clone();
    let world_config = &deps.config.world;
    let mut keep_alive = tokio::time::interval(KEEP_ALIVE_INTERVAL);
    keep_alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    keep_alive.tick().await;
    let mut chunk_unload_sweep = tokio::time::interval(CHUNK_UNLOAD_SWEEP_INTERVAL);
    chunk_unload_sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    chunk_unload_sweep.tick().await;
    let mut chunk_send_tick = tokio::time::interval(CHUNK_SEND_TICK_INTERVAL);
    chunk_send_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    chunk_send_tick.tick().await;
    let mut gameplay_tick = tokio::time::interval(GAMEPLAY_TICK_INTERVAL);
    gameplay_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    gameplay_tick.tick().await;
    let mut survival_tick = tokio::time::interval(SURVIVAL_TICK_INTERVAL);
    survival_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    survival_tick.tick().await;
    let mut world_time_tick = tokio::time::interval(WORLD_TIME_TICK_INTERVAL);
    world_time_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    world_time_tick.tick().await;
    let mut player_data_autosave = tokio::time::interval(MIN_PLAYER_DATA_AUTOSAVE_INTERVAL);
    player_data_autosave.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    player_data_autosave.tick().await;
    let mut pending_keep_alive: Option<i64> = None;
    let mut chat_rate_limit = ChatRateLimit::new(
        deps.config.player_messages.chat_rate_limit,
        deps.config.player_messages.chat_rate_window_secs,
    );
    let mut current_game_mode = world_config.game_mode;
    let simulation_distance = world_config.simulation_distance.max(1);
    let mut position = session.player.position;
    let mut initial_cluster_entity_view_sent = false;
    let mut last_input_flags = 0u8;
    let lobby = LobbyRuntime::new(&deps.config.lobby);
    let mut lobby_status = lobby.refresh_status().await;
    let mut lobby_status_refresh = lobby.status_refresh_interval().map(tokio::time::interval);
    if let Some(interval) = lobby_status_refresh.as_mut() {
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
    }
    let mut lobby_broadcast = lobby.broadcast_interval().map(tokio::time::interval);
    if let Some(interval) = lobby_broadcast.as_mut() {
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
    }
    let mut lobby_broadcast_index = 0usize;
    let mut lobby_menu_open = false;
    // TODO(play-gameplay)：菜单点击处理接入后恢复使用
    let _menus = MenuRuntime::new(&deps.config.menus);
    let mut active_config_menu: Option<String> = None;
    let mut players_hidden = false;
    let mut visible_player_entities = deps
        .players
        .list_except(profile.uuid)
        .into_iter()
        .filter(|player| player.dimension == play_dimension)
        .filter(|player| {
            within_horizontal_distance(
                position,
                player.position,
                deps.entity_rendering.player_distance,
            )
        })
        .map(|player| player.profile.uuid)
        .collect::<HashSet<_>>();
    let _ = &mut active_config_menu;
    let _ = &mut players_hidden;
    let _ = &mut lobby_menu_open;
    lobby.show_boss_bar(sink).await?;
    lobby.update_boss_bar_status(sink, &lobby_status).await?;
    if let Some(request) = lobby.proxy_server_list_request(&default_proxy_config()) {
        sink.send(request).await?;
    }
    let (chunk_sender, mut chunk_receiver) = tokio::sync::mpsc::unbounded_channel();
    chunk_state.refresh_pending_chunks();
    let fluid = FluidRuntime::new();
    let fluid_seeds = chunk_state
        .send_center_chunk_first(sink, &world, deps.plugins)
        .await?;
    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
    let fluid_seeds = chunk_state
        .send_remaining_initial_chunks(sink, &world, deps.plugins)
        .await?;
    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
    chunk_state.start_next_chunk_load(&world, Some(&chunk_sender), sink.compression_threshold());

    let session_entity_id = session.player.entity_id;
    let session_profile_id = profile.uuid;

    fn tick_context<'a>(
        profile_id: uuid::Uuid,
        entity_id: i32,
        dimension: &'a str,
        position: EntityPosition,
        game_mode: GameMode,
    ) -> SessionTickContext<'a> {
        SessionTickContext {
            profile_id,
            entity_id,
            dimension,
            position,
            game_mode,
        }
    }

    let result: Result<()> = async {
        loop {
            tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_ok() && *shutdown.borrow() {
                    log::debug!("closing play session because server shutdown was requested");
                    break Ok(());
                }
            }
            loaded_chunk = chunk_receiver.recv(), if chunk_state.has_loading_chunks() => {
                let Some(loaded_chunk) = loaded_chunk else {
                    return Err(PlayError::msg("chunk load task channel closed"));
                };
                chunk_state.queue_loaded_chunk(loaded_chunk);
                chunk_state.start_next_chunk_load(&world, Some(&chunk_sender), sink.compression_threshold());
                let fluid_seeds = chunk_state
                    .send_ready_chunks(sink, &world, deps.plugins, &chunk_sender, false)
                    .await?;
                fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                send_initial_cluster_entity_view_after_chunks(
                    sink,
                    deps.cluster,
                    &mut initial_cluster_entity_view_sent,
                    &mut session.player,
                    &play_dimension,
                    position,
                    &deps.entity_rendering,
                    simulation_distance,
                    &chunk_state,
                )
                .await?;
            }
            _ = chunk_send_tick.tick(), if chunk_state.has_ready_chunks() => {
                let fluid_seeds = chunk_state
                    .send_ready_chunks(sink, &world, deps.plugins, &chunk_sender, true)
                    .await?;
                fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
            }
            _ = chunk_unload_sweep.tick(), if chunk_state.has_pending_unloads() => {
                let unloaded = chunk_state
                    .unload_expired_chunks(sink, deps.plugins, std::time::Instant::now())
                    .await?;
                if unloaded > 0 {
                    log::debug!("delayed chunk unload completed: count={unloaded}");
                }
            }
            _ = pending_chunk_center_update_sleep(&chunk_state), if chunk_state.has_pending_center_update() => {
                if chunk_state
                    .apply_due_center_update(sink, &chunk_sender, &world, position.x, position.z)
                    .await?
                {
                    log::debug!("delayed chunk center update applied");
                }
            }
            _ = survival_tick.tick() => {
                let ctx = tick_context(session_profile_id, session_entity_id, &play_dimension, position, current_game_mode);
                if let Some(message) = deps.gameplay.survival_tick(&ctx)? {
                    deps.gameplay.handle_death(&ctx, &message)?;
                }
            }
            _ = gameplay_tick.tick() => {
                let ctx = tick_context(session_profile_id, session_entity_id, &play_dimension, position, current_game_mode);
                deps.gameplay.plugin_player_tick(
                    &ctx,
                    GAMEPLAY_TICK_INTERVAL.as_millis() as u64,
                )?;
                deps.gameplay.gameplay_tick(&ctx)?;
            }
            _ = world_time_tick.tick() => {
                let game_time = deps.world_rules.tick_dimension_time(&play_dimension, world_config.day_ticks);
                sink.send(qexed_protocol::to_client::play::set_time::SetTime {
                    game_time,
                    clock_updates: Vec::new(),
                })
                .await?;
                sink.flush().await?;
            }
            _ = player_data_autosave.tick() => {
                save_player_runtime(
                    deps.player_data,
                    _player_data_lock,
                    saved_player,
                    &play_dimension,
                    position,
                    deps.gameplay.inventory_snapshot(),
                    profile.uuid,
                    "autosave",
                )
                .await;
                deps.gameplay.flush_world_writes();
            }
            event = session.receiver.recv() => {
                let Some(event) = event else {
                    continue;
                };
                if let PlayerEvent::Damage {
                    profile_id: target_id,
                    amount,
                    kind,
                    source_entity_id,
                    source_position,
                    knockback,
                    ..
                } = event
                {
                    if target_id == profile.uuid {
                        let ctx = tick_context(session_profile_id, session_entity_id, &play_dimension, position, current_game_mode);
                        if let Some(message) = deps.gameplay.apply_damage(
                            &ctx,
                            amount,
                            kind,
                            source_entity_id,
                            source_position,
                            knockback,
                        )? {
                            deps.gameplay.handle_death(&ctx, &message)?;
                        }
                    }
                    continue;
                }
                if let PlayerEvent::GameModeChanged {
                    profile_id: target_id,
                    username: _,
                    game_mode,
                } = event
                {
                    if target_id == profile.uuid {
                        if let Some(next_mode) = GameMode::from_protocol_id(game_mode) {
                            current_game_mode = next_mode;
                            session.player.game_mode = game_mode;
                            sink.send(GameEvent {
                                event: 3,
                                param: game_mode as f32,
                            })
                            .await?;
                            sink.send(ClientboundPlayerAbilities {
                                flags: player_ability_flags(
                                    current_game_mode,
                                    world_config.allow_flight,
                                ),
                                flying_speed: 0.05,
                                walking_speed: 0.1,
                            })
                            .await?;
                            sink.send(PlayerInfoUpdate {
                                actions: PlayerInfoActions(PlayerInfoActions::UPDATE_GAME_MODE),
                                entries: vec![PlayerInfoEntry {
                                    profile_id: profile.uuid,
                                    game_mode: VarInt(game_mode),
                                    ..PlayerInfoEntry::default()
                                }],
                            })
                            .await?;
                            sink.flush().await?;
                        }
                    }
                    continue;
                }
                if event_is_self(&event, profile.uuid) {
                    continue;
                }
                if players_hidden && event_affects_player_entity(&event) {
                    continue;
                }
                let event_packets = filtered_player_event_packets(
                    &event,
                    deps.players,
                    deps.player_entity_type,
                    &play_dimension,
                    position,
                    deps.entity_rendering.player_distance,
                    block_update_distance(world_config.view_distance.max(1)),
                    &mut visible_player_entities,
                )?;
                for packet in event_packets {
                    sink.send_raw(packet).await?;
                }
                if let Some(message) = player_event_message(&deps.config.player_messages, &event) {
                    sink.send(SystemChat {
                        content: text_component(message),
                        overlay: false,
                    })
                    .await?;
                }
                sink.flush().await?;
            }
            packet = packets.read_packet() => {
                let Some(mut payload) = packet.map_err(PlayError::from)? else {
                    break Ok(());
                };

                let packet_id = read_packet_id(&mut payload)?;
                if packet_id == AcceptTeleportation::ID {
                    let teleport = decode_payload::<AcceptTeleportation>(&mut payload)?;
                    log::debug!("client accepted teleport: teleport_id={}", teleport.teleport_id.0);
                    continue;
                }

                if packet_id == ServerboundKeepAlive::ID {
                    let keep_alive = decode_payload::<ServerboundKeepAlive>(&mut payload)?;
                    match pending_keep_alive {
                        Some(expected) if keep_alive.keep_alive_id == expected => {
                            log::debug!("client responded KeepAlive: id={expected}");
                            pending_keep_alive = None;
                        }
                        Some(expected) => {
                            return Err(PlayError::msg(
                                qexed_language::t("qexed.play.keepalive.mismatch")
                                    .replace("%{expected}", &expected.to_string())
                                    .replace(
                                        "%{actual}",
                                        &keep_alive.keep_alive_id.to_string(),
                                    ),
                            ));
                        }
                        None => {
                            log::trace!("ignored unsolicited client KeepAlive: id={}", keep_alive.keep_alive_id);
                        }
                    }
                    continue;
                }

                if packet_id == ChunkBatchReceived::ID {
                    let batch = decode_payload::<ChunkBatchReceived>(&mut payload)?;
                    chunk_state.on_chunk_batch_received(batch.desired_chunks_per_tick);
                    let fluid_seeds = chunk_state
                        .send_ready_chunks(sink, &world, deps.plugins, &chunk_sender, false)
                        .await?;
                    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                    send_initial_cluster_entity_view_after_chunks(
                        sink,
                        deps.cluster,
                        &mut initial_cluster_entity_view_sent,
                        &mut session.player,
                        &play_dimension,
                        position,
                        &deps.entity_rendering,
                        simulation_distance,
                        &chunk_state,
                    )
                    .await?;
                    log::debug!(
                        "client acknowledged chunk batch, desired rate: {} chunks/tick",
                        batch.desired_chunks_per_tick
                    );
                    continue;
                }

                if packet_id == PlayerLoaded::ID {
                    let _loaded = decode_payload::<PlayerLoaded>(&mut payload)?;
                    let fluid_seeds = chunk_state
                        .send_ready_chunks(sink, &world, deps.plugins, &chunk_sender, false)
                        .await?;
                    fluid.enqueue_fluid_seeds(&play_dimension, fluid_seeds);
                    send_initial_cluster_entity_view_after_chunks(
                        sink,
                        deps.cluster,
                        &mut initial_cluster_entity_view_sent,
                        &mut session.player,
                        &play_dimension,
                        position,
                        &deps.entity_rendering,
                        simulation_distance,
                        &chunk_state,
                    )
                    .await?;
                    log::debug!("client reported player loaded: player={}", profile.username);
                    continue;
                }

                if packet_id == Pos::ID {
                    let movement = decode_payload::<Pos>(&mut payload)?;
                    let previous = position;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.on_ground = movement.on_ground;
                    apply_movement(
                        sink, deps, &world, &mut chunk_state, &chunk_sender,
                        &mut play_dimension, &mut position, previous,
                        &mut visible_player_entities, &session, profile,
                    ).await?;
                    continue;
                }

                if packet_id == PosRot::ID {
                    let movement = decode_payload::<PosRot>(&mut payload)?;
                    let previous = position;
                    position.x = movement.x;
                    position.y = movement.y;
                    position.z = movement.z;
                    position.yaw = unpack_i8_angle(movement.yaw);
                    position.pitch = unpack_i8_angle(movement.pitch);
                    position.on_ground = movement.on_ground;
                    apply_movement(
                        sink, deps, &world, &mut chunk_state, &chunk_sender,
                        &mut play_dimension, &mut position, previous,
                        &mut visible_player_entities, &session, profile,
                    ).await?;
                    continue;
                }

                if packet_id == Rot::ID {
                    let movement = decode_payload::<Rot>(&mut payload)?;
                    position.yaw = unpack_i8_angle(movement.yaw);
                    position.pitch = unpack_i8_angle(movement.pitch);
                    position.on_ground = movement.on_ground;
                    deps.players.update_position(profile.uuid, position);
                    session.player.position = position;
                    continue;
                }

                if packet_id == StatusOnly::ID {
                    let movement = decode_payload::<StatusOnly>(&mut payload)?;
                    position.on_ground = movement.flags & MOVE_FLAG_ON_GROUND != 0;
                    deps.players.update_position(profile.uuid, position);
                    session.player.position = position;
                    continue;
                }

                if packet_id == qexed_protocol::to_server::play::player_input::PlayerInput::ID {
                    let input = decode_payload::<qexed_protocol::to_server::play::player_input::PlayerInput>(&mut payload)?;
                    let previous_flags = last_input_flags;
                    last_input_flags = input.flags;
                    let ctx = tick_context(session_profile_id, session_entity_id, &play_dimension, position, current_game_mode);
                    deps.gameplay.player_input(&ctx, previous_flags, input.flags)?;
                    continue;
                }

                if packet_id == ChatAck::ID {
                    let ack = decode_payload::<ChatAck>(&mut payload)?;
                    deps.secure_chat.apply_ack(ack.offset)?;
                    continue;
                }

                if packet_id == qexed_protocol::to_server::play::chat::Chat::ID {
                    let chat = decode_payload::<qexed_protocol::to_server::play::chat::Chat>(&mut payload)?;
                    if let Some(message) = validate_player_chat_message(&mut chat_rate_limit, &chat.message) {
                        sink.send(SystemChat {
                            content: text_component(message),
                            overlay: false,
                        })
                        .await?;
                        sink.flush().await?;
                        tokio::task::yield_now().await;
                        continue;
                    }
                    let filtered_message = match deps.chat_filter.check_chat(&chat.message)? {
                        FilterAction::Allow(message) => message,
                        FilterAction::Block { reason } => {
                            sink.send(SystemChat {
                                content: text_component(reason),
                                overlay: false,
                            })
                            .await?;
                            sink.flush().await?;
                            tokio::task::yield_now().await;
                            continue;
                        }
                    };
                    // 优先走安全聊天校验（返回已编码广播包），否则本地 system chat 广播。
                    if let Some(packet) = deps.secure_chat.verify_message(&chat)? {
                        deps.players
                            .broadcast_packets_except(profile.uuid, vec![packet.clone()]);
                        sink.send_raw(packet).await?;
                    } else {
                        let packet = SystemChat {
                            content: text_component(format!("<{}> {}", profile.username, filtered_message)),
                            overlay: false,
                        };
                        let encoded = qexed_player::packet_bytes(packet.clone())?;
                        deps.players
                            .broadcast_packets_except(profile.uuid, vec![encoded]);
                        sink.send(packet).await?;
                    }
                    sink.flush().await?;
                    tokio::task::yield_now().await;
                    continue;
                }

                if packet_id == qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate::ID {
                    let update = decode_payload::<qexed_protocol::to_server::play::chat_session_update::ChatSessionUpdate>(&mut payload)?;
                    log::debug!("received chat session update");
                    deps.secure_chat.verify_session_update(&update.chat_session)?;
                    sink.send(PlayerInfoUpdate {
                        actions: PlayerInfoActions(PlayerInfoActions::INITIALIZE_CHAT),
                        entries: vec![PlayerInfoEntry {
                            profile_id: profile.uuid,
                            chat_session: Some(update.chat_session),
                            ..PlayerInfoEntry::default()
                        }],
                    }).await?;
                    sink.flush().await?;
                    tokio::task::yield_now().await;
                    continue;
                }

                if packet_id == ChatCommand::ID {
                    let command = decode_payload::<ChatCommand>(&mut payload)?;
                    log::debug!("received chat command: /{}", command.command);
                    deps.player_audit.record(
                        profile.uuid,
                        "command",
                        &format!("/{}", command.command),
                    );
                    let ctx = tick_context(session_profile_id, session_entity_id, &play_dimension, position, current_game_mode);
                    deps.gameplay.handle_command(&ctx, &command.command)?;
                    sink.flush().await?;
                    tokio::task::yield_now().await;
                    continue;
                }

                log::trace!("skipped unhandled Play serverbound packet ID: {packet_id}");
            }
            _ = keep_alive.tick() => {
                if let Some(expected) = pending_keep_alive {
                    return Err(PlayError::msg(
                        qexed_language::t("qexed.play.keepalive.timeout")
                            .replace("%{id}", &expected.to_string()),
                    ));
                }

                let keep_alive_id = keep_alive_id();
                sink.send(ClientboundKeepAlive { keep_alive_id }).await?;
                sink.flush().await?;
                pending_keep_alive = Some(keep_alive_id);
                log::debug!("sent Play KeepAlive: id={keep_alive_id}");
            }
            _ = async {
                if let Some(interval) = lobby_status_refresh.as_mut() {
                    interval.tick().await;
                }
            }, if lobby_status_refresh.is_some() => {
                let refreshed = lobby.refresh_status().await;
                for (server, status) in refreshed.changed_servers_since(&lobby_status) {
                    log::info!("lobby backend status changed: server={server}, status={status:?}");
                }
                lobby_status = refreshed;
                lobby.update_boss_bar_status(sink, &lobby_status).await?;
                for packet in deps.gameplay.sidebar_packets()? {
                    sink.send_raw(packet).await?;
                }
                if lobby_menu_open {
                    lobby.refresh_open_menu(sink, deps.items, &lobby_status).await?;
                }
                sink.flush().await?;
            }
            _ = async {
                if let Some(interval) = lobby_broadcast.as_mut() {
                    interval.tick().await;
                }
            }, if lobby_broadcast.is_some() => {
                if let Some(message) = lobby.broadcast_message(lobby_broadcast_index, &lobby_status) {
                    let progress = lobby_broadcast_progress(
                        lobby_broadcast_index,
                        deps.config.lobby.broadcast.messages.len(),
                    );
                    lobby.update_boss_bar(sink, &message, progress).await?;
                    sink.send(SystemChat {
                        content: text_component(message),
                        overlay: false,
                    }).await?;
                    sink.flush().await?;
                    lobby_broadcast_index = lobby_broadcast_index.wrapping_add(1);
                }
            }
            }
        }
    }
    .await;

    if let Err(err) = lobby.remove_boss_bar(sink).await {
        log::debug!("failed to remove lobby boss bar before disconnect: {err}");
    }
    save_player_runtime(
        deps.player_data,
        _player_data_lock,
        saved_player,
        &play_dimension,
        position,
        deps.gameplay.inventory_snapshot(),
        profile.uuid,
        "disconnect",
    )
    .await;
    deps.gameplay.flush_world_writes();

    result
}

/// 移动后处理（v4 各 move 分支的公共尾部：survival -> plugin step/move ->
/// chunk center -> 玩家管理器同步 -> 集群视图 -> 可见玩家 -> 掉落物拾取）。
#[allow(clippy::too_many_arguments)]
async fn apply_movement<W>(
    sink: &mut PacketSink<W>,
    deps: &PlaySessionDeps<'_>,
    world: &SharedWorld,
    chunk_state: &mut ChunkSendState,
    chunk_sender: &tokio::sync::mpsc::UnboundedSender<chunks::ChunkLoadResult>,
    play_dimension: &mut String,
    position: &mut EntityPosition,
    previous: EntityPosition,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
    session: &PlayerSession,
    profile: &qexed_packet::net_types::GameProfile,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let ctx = SessionTickContext {
        profile_id: profile.uuid,
        entity_id: session.player.entity_id,
        dimension: play_dimension,
        position: *position,
        game_mode: deps.config.world.game_mode,
    };
    deps.gameplay.survival_movement(&ctx, previous)?;
    deps.gameplay.player_block_step(&ctx)?;
    deps.gameplay.player_move(&ctx, previous)?;
    chunk_state
        .update_center(sink, chunk_sender, world, position.x, position.z)
        .await?;
    deps.players.update_position(profile.uuid, *position);
    if let Some(cluster) = deps.cluster {
        let mut player = session.player.clone();
        player.position = *position;
        player.dimension = play_dimension.clone();
        let packets = cluster.spawn_view_for_player(
            &player,
            &deps.entity_rendering,
            simulation_distance_of(deps),
        );
        for packet in packets {
            sink.send_raw(packet).await?;
        }
    }
    if !players_hidden_of(&visible_player_entities) && players_visible_mode(deps) {
        refresh_visible_players(
            sink,
            deps.players,
            profile.uuid,
            deps.player_entity_type,
            play_dimension,
            *position,
            deps.entity_rendering.player_distance,
            visible_player_entities,
        )
        .await?;
    }
    deps.gameplay.collect_nearby_drops(&ctx)?;
    Ok(())
}

fn simulation_distance_of(deps: &PlaySessionDeps<'_>) -> i32 {
    deps.config.world.simulation_distance.max(1)
}

fn players_hidden_of(_visible: &HashSet<uuid::Uuid>) -> bool {
    false
}

fn players_visible_mode(_deps: &PlaySessionDeps<'_>) -> bool {
    true
}

fn default_proxy_config() -> crate::config::ServerProxyConfig {
    crate::config::ServerProxyConfig::default()
}

fn unpack_i8_angle(packed: i8) -> f32 {
    (packed as f32) * 360.0 / 256.0
}

fn read_packet_id(payload: &mut bytes::BytesMut) -> Result<i32> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut id = VarInt::default();
    id.deserialize(&mut reader)?;
    Ok(id.0)
}

fn decode_payload<T: qexed_packet::Packet>(
    payload: &mut bytes::BytesMut,
) -> Result<T> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet = T::default();
    packet.deserialize(&mut reader)?;
    Ok(packet)
}

async fn save_player_runtime(
    player_data: &PlayerDataManager,
    player_data_lock: &PlayerDataLockGuard,
    saved_player: &mut PlayerData,
    play_dimension: &str,
    position: EntityPosition,
    inventory: qexed_player::StoredInventory,
    profile_id: uuid::Uuid,
    reason: &str,
) {
    saved_player.update_runtime(
        play_dimension,
        position,
        &inventory,
        saved_player.survival.clone(),
    );
    if let Err(err) = player_data
        .locked_save(player_data_lock, saved_player)
        .await
    {
        log::warn!(
            "{}",
            qexed_language::t("qexed.play.data.save_failed")
                .replace("%{uuid}", &profile_id.to_string())
                .replace("%{reason}", reason)
                .replace("%{error}", &err.to_string())
        );
    } else {
        log::debug!(
            "{}",
            qexed_language::t("qexed.play.data.saved")
                .replace("%{uuid}", &profile_id.to_string())
                .replace("%{reason}", reason)
        );
    }
}

fn lobby_broadcast_progress(index: usize, message_count: usize) -> f32 {
    if message_count == 0 {
        return 1.0;
    }
    ((index % message_count) + 1) as f32 / message_count as f32
}

fn pending_chunk_center_update_sleep(chunk_state: &ChunkSendState) -> tokio::time::Sleep {
    match chunk_state.pending_center_update_deadline() {
        Some(deadline) => tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)),
        // 无待处理更新时永远沉睡（分支条件保证不会走到）。
        None => tokio::time::sleep(std::time::Duration::from_secs(86_400 * 365)),
    }
}

async fn send_initial_cluster_entity_view_after_chunks<W>(
    sink: &mut PacketSink<W>,
    cluster: Option<&dyn ClusterEntityView>,
    sent: &mut bool,
    player: &mut OnlinePlayer,
    dimension: &str,
    position: EntityPosition,
    _rendering: &qexed_entities::EntityRendering,
    simulation_distance: i32,
    chunk_state: &ChunkSendState,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    if *sent || !chunk_state.initial_view_complete() {
        return Ok(());
    }
    *sent = true;
    let Some(cluster) = cluster else {
        return Ok(());
    };

    player.position = position;
    player.dimension = dimension.to_string();
    let packets = cluster.spawn_view_for_player(player, _rendering, simulation_distance);
    for packet in packets.iter().cloned() {
        sink.send_raw(packet).await?;
    }
    if !packets.is_empty() {
        log::debug!(
            "initial cluster entity view sent: player={}, packets={}",
            player.profile.username,
            packets.len()
        );
        sink.flush().await?;
    }
    Ok(())
}

pub async fn refresh_visible_players<W>(
    sink: &mut PacketSink<W>,
    players: &PlayerManager,
    actor: uuid::Uuid,
    player_entity_type: i32,
    dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut still_visible = HashSet::new();
    for player in players.list_except(actor) {
        if player.dimension != dimension {
            continue;
        }
        if !within_horizontal_distance(viewer_position, player.position, render_distance) {
            continue;
        }
        still_visible.insert(player.profile.uuid);
        if visible_player_entities.contains(&player.profile.uuid) {
            continue;
        }
        for packet in qexed_player::spawn_player_packets(&player, player_entity_type)? {
            sink.send_raw(packet).await?;
        }
    }

    let to_remove = visible_player_entities
        .difference(&still_visible)
        .copied()
        .collect::<Vec<_>>();
    for profile_id in to_remove {
        visible_player_entities.remove(&profile_id);
        if let Some(player) = players.player_by_uuid(profile_id) {
            sink.send(RemoveEntities::one(player.entity_id)).await?;
        }
    }
    Ok(())
}

pub fn within_horizontal_distance(
    left: EntityPosition,
    right: EntityPosition,
    distance: f64,
) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    (dx * dx + dz * dz) <= distance * distance
}

pub fn block_update_distance(view_distance_chunks: i32) -> f64 {
    f64::from(view_distance_chunks.max(1) * 16 + 16)
}

fn block_position_within_horizontal_distance(
    viewer: EntityPosition,
    block: &BlockPosition,
    distance: f64,
) -> bool {
    if distance <= 0.0 {
        return false;
    }
    let dx = viewer.x - (f64::from(block.x) + 0.5);
    let dz = viewer.z - (f64::from(block.z) + 0.5);
    (dx * dx + dz * dz) <= distance * distance
}

/// 方块变更补发包（v4 block_update_packets；单块 BlockUpdate，多块 SectionBlocksUpdate）。
pub fn block_update_packets(
    changes: impl IntoIterator<Item = (BlockPosition, i32)>,
) -> Result<Vec<bytes::Bytes>> {
    let mut by_section: BTreeMap<(i32, i32, i32), Vec<(BlockPosition, i32)>> = BTreeMap::new();
    for (position, block_state) in changes {
        by_section
            .entry((
                position.x.div_euclid(16),
                position.y.div_euclid(16),
                position.z.div_euclid(16),
            ))
            .or_default()
            .push((position, block_state));
    }

    let mut packets = Vec::new();
    for ((section_x, section_y, section_z), mut section_changes) in by_section {
        if section_changes.len() == 1 {
            let (position, block_state) = section_changes.pop().expect("one block change");
            packets.push(qexed_player::packet_bytes(
                qexed_protocol::to_client::play::block_update::BlockUpdate {
                    location: position,
                    block_state: VarInt(block_state),
                },
            )?);
            continue;
        }
        section_changes.sort_by_key(|(position, _)| {
            (
                position.y.rem_euclid(16),
                position.z.rem_euclid(16),
                position.x.rem_euclid(16),
            )
        });
        let blocks = section_changes
            .into_iter()
            .map(|(position, block_state)| {
                VarLong((i64::from(block_state) << 12) | i64::from(section_relative_pos(&position)))
            })
            .collect();
        packets.push(qexed_player::packet_bytes(
            qexed_protocol::to_client::play::section_blocks_update::SectionBlocksUpdate {
                section_position: section_position(section_x, section_y, section_z),
                blocks,
            },
        )?);
    }
    Ok(packets)
}

fn section_relative_pos(position: &BlockPosition) -> i32 {
    (position.x.rem_euclid(16) << 8) | (position.z.rem_euclid(16) << 4) | position.y.rem_euclid(16)
}

fn section_position(section_x: i32, section_y: i32, section_z: i32) -> i64 {
    ((i64::from(section_x) & 0x3f_ffff) << 42)
        | ((i64::from(section_z) & 0x3f_ffff) << 20)
        | (i64::from(section_y) & 0x0f_ffff)
}

fn event_affects_player_entity(event: &PlayerEvent) -> bool {
    matches!(
        event,
        PlayerEvent::Joined(_)
            | PlayerEvent::Left { .. }
            | PlayerEvent::Moved { .. }
            | PlayerEvent::DimensionChanged { .. }
            | PlayerEvent::SetEquipmentChanged { .. }
    )
}

/// 其他玩家事件过滤（v4 filtered_player_event_packets）。
fn filtered_player_event_packets(
    event: &PlayerEvent,
    players: &PlayerManager,
    player_entity_type: i32,
    viewer_dimension: &str,
    viewer_position: EntityPosition,
    render_distance: f64,
    block_update_distance: f64,
    visible_player_entities: &mut HashSet<uuid::Uuid>,
) -> Result<Vec<bytes::Bytes>> {
    match event {
        PlayerEvent::Joined(player) => {
            if player.dimension != viewer_dimension {
                return Ok(vec![qexed_player::packet_bytes(PlayerInfoUpdate {
                    actions: PlayerInfoActions::player_initializing(),
                    entries: vec![PlayerInfoEntry::from_profile(&player.profile, 1)],
                })
                .map_err(PlayError::from)?]);
            }
            if !within_horizontal_distance(viewer_position, player.position, render_distance) {
                visible_player_entities.remove(&player.profile.uuid);
                return Ok(Vec::new());
            }
            visible_player_entities.insert(player.profile.uuid);
            qexed_player::spawn_player_packets(player, player_entity_type)
                .map_err(PlayError::from)
        }
        PlayerEvent::Left {
            profile_id,
            entity_id,
            dimension,
            ..
        } => {
            let mut packets = Vec::new();
            if dimension == viewer_dimension && visible_player_entities.remove(profile_id) {
                packets.push(qexed_player::packet_bytes(RemoveEntities::one(*entity_id))?);
            }
            packets.push(qexed_player::packet_bytes(
                qexed_protocol::to_client::play::player_info_remove::PlayerInfoRemove::one(
                    *profile_id,
                ),
            )?);
            Ok(packets)
        }
        PlayerEvent::Moved {
            profile_id,
            entity_id,
            dimension,
            position,
        } => {
            if dimension != viewer_dimension {
                if visible_player_entities.remove(profile_id) {
                    return Ok(vec![qexed_player::packet_bytes(RemoveEntities::one(
                        *entity_id,
                    ))?]);
                }
                return Ok(Vec::new());
            }

            let in_range = within_horizontal_distance(viewer_position, *position, render_distance);
            let was_visible = visible_player_entities.contains(profile_id);
            if in_range && !was_visible {
                if let Some(player) = players.player_by_uuid(*profile_id) {
                    visible_player_entities.insert(*profile_id);
                    return qexed_player::spawn_player_packets(&player, player_entity_type)
                        .map_err(PlayError::from);
                }
            }
            if in_range {
                visible_player_entities.insert(*profile_id);
                return event
                    .packets(player_entity_type, viewer_dimension)
                    .map_err(PlayError::from);
            }
            if was_visible {
                visible_player_entities.remove(profile_id);
                return Ok(vec![qexed_player::packet_bytes(RemoveEntities::one(
                    *entity_id,
                ))?]);
            }
            Ok(Vec::new())
        }
        PlayerEvent::DimensionChanged {
            profile_id,
            entity_id,
            old_dimension,
            player,
        } => {
            let mut packets = Vec::new();
            if old_dimension == viewer_dimension && visible_player_entities.remove(profile_id) {
                packets.push(qexed_player::packet_bytes(RemoveEntities::one(*entity_id))?);
            }
            if player.dimension == viewer_dimension
                && within_horizontal_distance(viewer_position, player.position, render_distance)
            {
                visible_player_entities.insert(*profile_id);
                packets.extend(
                    qexed_player::spawn_player_packets(player, player_entity_type)
                        .map_err(PlayError::from)?,
                );
            }
            Ok(packets)
        }
        PlayerEvent::SetEquipmentChanged {
            profile_id,
            dimension,
            ..
        }
        | PlayerEvent::Animation {
            profile_id,
            dimension,
            ..
        } => {
            if dimension != viewer_dimension || !visible_player_entities.contains(profile_id) {
                return Ok(Vec::new());
            }
            event
                .packets(player_entity_type, viewer_dimension)
                .map_err(PlayError::from)
        }
        PlayerEvent::BlockChanged {
            dimension,
            position,
            ..
        } => {
            if dimension != viewer_dimension
                || !block_position_within_horizontal_distance(
                    viewer_position,
                    position,
                    block_update_distance,
                )
            {
                return Ok(Vec::new());
            }
            event
                .packets(player_entity_type, viewer_dimension)
                .map_err(PlayError::from)
        }
        PlayerEvent::BlockChanges {
            dimension, changes, ..
        } => {
            if dimension != viewer_dimension {
                return Ok(Vec::new());
            }
            let visible_changes = changes
                .iter()
                .filter(|change| {
                    block_position_within_horizontal_distance(
                        viewer_position,
                        &change.position,
                        block_update_distance,
                    )
                })
                .map(|change| (change.position.clone(), change.block_state))
                .collect::<Vec<_>>();
            block_update_packets(visible_changes)
        }
        _ => event
            .packets(player_entity_type, viewer_dimension)
            .map_err(PlayError::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_rate_limit_blocks_bursts() {
        let mut limit = ChatRateLimit::new(3, 60);
        for _ in 0..3 {
            assert!(validate_player_chat_message(&mut limit, "hi").is_none());
        }
        assert!(validate_player_chat_message(&mut limit, "hi").is_some());
    }

    #[test]
    fn broadcast_progress_advances() {
        assert!((lobby_broadcast_progress(0, 4) - 0.25).abs() < f32::EPSILON);
        assert!((lobby_broadcast_progress(3, 4) - 1.0).abs() < f32::EPSILON);
        assert_eq!(lobby_broadcast_progress(0, 0), 1.0);
    }

    #[test]
    fn i8_angle_rounding_covers_full_circle() {
        assert!((unpack_i8_angle(0) - 0.0).abs() < f32::EPSILON);
        assert!((unpack_i8_angle(64) - 90.0).abs() < 0.01);
        assert!((unpack_i8_angle(-64) + 90.0).abs() < 0.01);
    }

    #[test]
    fn section_position_packs_bits() {
        assert_eq!(section_position(0, 0, 0), 0);
        assert_eq!(section_position(1, 0, 0), 1 << 42);
        assert_eq!(section_position(0, 1, 0), 1);
        assert_eq!(section_position(0, 0, 1), 1 << 20);
    }

    #[test]
    fn block_update_packets_group_by_section() {
        let changes = vec![
            (BlockPosition { x: 0, y: 64, z: 0 }, 1),
            (BlockPosition { x: 0, y: 65, z: 0 }, 2),
            (BlockPosition { x: 32, y: 64, z: 0 }, 3),
        ];
        let packets = block_update_packets(changes).unwrap();
        // 同 section 两块合并为 1 包 + 另一 section 单块 1 包
        assert_eq!(packets.len(), 2);
    }
}
