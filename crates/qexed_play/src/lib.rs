//! PLAY 阶段：入服引导、主循环、以及把具体行为分发到各行为 crate。
//!
//! 依赖方向（自上而下）：
//! ```text
//! qexed_server → qexed_play → [qexed_chat, 未来的 qexed_entities / qexed_world / ...]
//!                            → qexed_protocol / qexed_packet / qexed_tcp_connect
//! ```
//!
//! 本 crate 只做三件事：
//! 1. 会话状态（[session::PlayPlayer]）
//! 2. 入服引导（[bootstrap]）与主循环（[handle_play]）
//! 3. 按 packet id 分发到行为 crate（聊天 → `qexed_chat`，其余暂记日志）

pub mod bootstrap;
pub mod chunk_worker;
pub mod config;
pub mod error;
pub mod jev_bridge;
pub mod session;

use qexed_packet::{Packet as _, PacketReader, net_types::VarInt};
use qexed_protocol::to_client;
use qexed_protocol::to_client::play::forget_level_chunk::ForgetLevelChunk;
use qexed_protocol::to_server::play;
use qexed_tcp_connect::{PacketSink, PacketStream};
use tokio::time::{Duration, Instant, MissedTickBehavior};

use qexed_command::CommandError;
use qexed_stat::counter::StatCounter;
use qexed_stat::registry::IdMaps;

use std::sync::{Arc, Mutex};

use crate::bootstrap::CommandEffect;
use crate::error::PlayError;
use crate::jev_bridge::JevChatOutcome;
use crate::session::PlayPlayer;

/// PLAY 主循环的心跳参数：15s 发一次，30s 未回应判超时（对齐 vanilla）。
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(30);
/// 两次包之间的最大静默读超时；超时未收到任何包则断开（防死连接挂资源）。
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// 实体 AI tick 节拍（v4 host 以 100ms 驱动实体 AI）。
const AI_TICK_INTERVAL: Duration = Duration::from_millis(100);
const AI_TICK_INTERVAL_MS: u64 = 100;

/// 处理 PLAY 阶段直到客户端断开。返回会话统计。
///
/// `chat_config` 由调用方（qexed_server）加载后传入；聊天行为委托给
/// `qexed_chat::ChatMessageHandler`。
pub async fn handle_play<R, W>(
    packets: &mut PacketStream<R>,
    sink: &mut PacketSink<W>,
    mut player: PlayPlayer,
    chat_config: &qexed_chat::config::ChatConfig,
    play_config: &config::PlayConfig,
) -> Result<session::PlaySessionStats, PlayError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let started = Instant::now();

    // ── 入服引导（Join Game → 初始状态流 → 位置同步） ──
    // 聊天行为先建好：入服时若已有会话（暂不会发生），bootstrap 一并下发。
    let mut chat = qexed_chat::ChatMessageHandler::new(chat_config, chat_config.secure_chat_enabled);
    bootstrap::send_join_game(sink, &player, chat_config, chat.chat_session()).await?;

    // ── 指令系统：建共享副作用队列与指令树，入服即下发 Declare Commands ──
    // 记分板权威状态与 op 队列本会话私有；单服单玩家，跨会话共享待 PlayerManager。
    let command_effects: Arc<Mutex<Vec<CommandEffect>>> = Arc::new(Mutex::new(Vec::new()));
    let scoreboard: Arc<Mutex<qexed_scoreboard::Scoreboard>> =
        Arc::new(Mutex::new(qexed_scoreboard::Scoreboard::new()));
    let scoreboard_ops: qexed_scoreboard::command::OpQueue = Arc::new(Mutex::new(Vec::new()));
    let mut commands = bootstrap::default_command_tree(
        command_effects.clone(),
        scoreboard_ops.clone(),
    );
    sink.send(bootstrap::command_tree_packet(&commands)).await?;

    // ── 统计：计数器 + 一次性解析 registries report 的 id 映射 ──
    let mut stats_counter = StatCounter::new();
    stats_counter.increment_custom("leave_game"); // 进服即 1，原版同款
    let id_maps = match IdMaps::load() {
        Ok(maps) => Some(maps),
        // 缓存缺失只降级统计屏幕（空数据），不影响入服
        Err(err) => {
            log::warn!("stat id maps unavailable, stats screen will be empty: {err}");
            None
        }
    };
    bootstrap::send_initial_state(sink, &player).await?;
    sink.flush().await?;
    log::info!(
        "player {} entered play phase (entity {})",
        player.display_name(),
        player.entity_id
    );

    // ── 插件桥：注册出站帧队列（v4 会话模型的 v6 版） ──
    // 插件的跨玩家动作（世界编辑/实体管理/给玩家的包）全部经 ActionHub
    // 队列下发；主循环 select 轮询该队列写入 sink，断开时注销。
    let hub = qexed_plugin::action_hub();
    let mut frame_rx = hub.register_player_session(&player.profile.username, player.entity_id);
    hub.replay_world_edits(&player.profile.username);
    let mut input_state_slot = qexed_plugin_api::PlayerInputState::default();

    // ── 插件事件：进服（对齐 v4 player_join：世界流送完成前即触发） ──
    fire_plugin_event(&qexed_plugin_api::PlayerJoinEvent::new(plugin_player_info(&player))).await;

    // ── 实体 AI 驱动节拍 ──
    let mut ai_tick = tokio::time::interval(AI_TICK_INTERVAL);
    ai_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ai_tick.tick().await; // 消费立即触发的首个 tick

    let mut stats = session::PlaySessionStats::default();

    // ── 出生区块流送：客户端卡在 loading terrain 就是在等这批数据 ──
    // Pumpkin：ChunkBatchStart → N×ChunkData → ChunkBatchFinished，客户端回
    // ChunkBatchReceived 后才回 PlayerLoaded。
    // 地形来源由配置决定（chunk_source）：
    //   empty     — 内存构造空区块（默认，超平占位，无外部依赖）
    //   chunk_tcp — 接入 chunk-summon 服务：存档命中读盘 / 未命中原版生成，
    //               玩家跑图任意坐标即取即得；服务不可达按
    //               chunk_tcp_fallback_empty 回退空区块或报错
    let (spawn_cx, spawn_cz) = player.chunk_pos();
    let mut view_center = (spawn_cx, spawn_cz);
    let chunk_source = &play_config.chunk_source;
    // TcpChunkSource 归会话所有：连接与跨 chunk 光照缓存随会话存活，
    // 增量跑图直接复用（命中缓存 / 补取邻居都不再重建连接）。
    // chunk_tcp 时出生圈与增量全部移交 ChunkWorker（独立任务）：网络+光照
    // 离开主循环，主循环只入队/收成品——出生加载与跑图都不再堵塞
    // keepalive/移动/交互，客户端加载地形期间会话仍全速处理上行包。
    let mut chunk_worker = chunk_worker::SessionWorker(None);
    match chunk_source {
        config::ChunkSource::Empty => {
            // 空区块是纯内存构造：直接内联发完（无 IO，无阻塞窗口）。
            match qexed_world::stream::send_spawn_chunks(
                sink,
                view_center,
                player.config.view_distance,
            )
            .await
            {
                Ok(count) => stats.chunks_sent = count as u64,
                // 空区块构造失败只可能是 sink 写错误（会由 ? 传播），防御性记录：
                Err(err) => log::warn!("spawn chunk stream failed: {err}"),
            }
        }
        config::ChunkSource::ChunkTcp => {
            let _spawn_span = qexed_profiler::profile_span!("world:spawn_stream");
            match qexed_world::tcp_source::TcpChunkSource::connect_addr(
                &play_config.chunk_tcp_addr,
            )
            .await
            {
                Ok(source) => {
                    let source =
                        source.with_fallback_to_empty(play_config.chunk_tcp_fallback_empty);
                    // 出生圈整体移交 worker：按近到远计划逐批产出，完成品由
                    // 主循环 select 分支下发；任务随后原地转增量模式。
                    chunk_worker = chunk_worker::SessionWorker(Some(
                        chunk_worker::ChunkWorker::spawn_stream(
                            source,
                            view_center,
                            player.config.view_distance,
                        ),
                    ));
                }
                Err(err) if play_config.chunk_tcp_fallback_empty => {
                    log::warn!(
                        "chunk-summon 服务不可达 ({}): {err}，回退空区块",
                        play_config.chunk_tcp_addr
                    );
                    match qexed_world::stream::send_spawn_chunks(
                        sink,
                        view_center,
                        player.config.view_distance,
                    )
                    .await
                    {
                        Ok(count) => stats.chunks_sent = count as u64,
                        Err(err) => log::warn!("spawn chunk stream failed: {err}"),
                    }
                }
                Err(err) => {
                    return Err(PlayError::ChunkTcp(format!(
                        "chunk-summon 服务不可达 ({}): {err}",
                        play_config.chunk_tcp_addr
                    )));
                }
            }
        }
    }

    // ── 增量跑图状态：已发集合 + 视野圈（与出生流送严格一致） ──
    let mut streaming = session::ChunkStreamingState::new(view_center, player.config.view_distance);

    // ── 心跳调度：v4 configuration.rs 的模式 ──
    let mut keepalive = tokio::time::interval(KEEPALIVE_INTERVAL);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    keepalive.tick().await; // 消费立即触发的首个 tick
    let mut pending_keepalive: Option<(i64, Instant)> = None;
    let mut next_keepalive_id: i64 = 1;

    loop {
        let read_deadline = match pending_keepalive {
            // 已发出心跳且未回应：以「发送时刻 + 超时」为限，
            // 期间的其他流量不重置窗口（对齐 vanilla 判定）
            Some((_, sent_at)) => sent_at + KEEPALIVE_TIMEOUT,
            None => Instant::now() + IDLE_TIMEOUT,
        };
        tokio::select! {
            _ = keepalive.tick() => {
                let id = next_keepalive_id;
                next_keepalive_id += 1;
                sink.send(to_client::play::keep_alive::KeepAlive { keep_alive_id: id })
                    .await?;
                if pending_keepalive.is_some() {
                    log::warn!("keepalive tick while previous one unanswered");
                }
                pending_keepalive = Some((id, Instant::now()));
                sink.flush().await?;
            }
            packet = tokio::time::timeout_at(read_deadline, packets.read_packet()) => {
                let Some(mut payload) = packet.map_err(|_| {
                    PlayError::Timeout("play read timeout (keepalive or idle)".into())
                })?? else {
                    break; // 客户端正常断开
                };
                stats.packets_received += 1;
                // 同步解码 → 异步落盘：PacketReader 含 &mut dyn Buf（非 Send），
                // 不能存活到 await 点之后
                let decoded = decode(&mut payload);
                let _pkt_span = qexed_profiler::profile_span!("pkt:process");
                if apply(
                    &mut player,
                    &mut chat,
                    &mut commands,
                    &command_effects,
                    &scoreboard,
                    &scoreboard_ops,
                    decoded,
                    sink,
                    &mut stats,
                    &mut pending_keepalive,
                    &mut view_center,
                    &mut streaming,
                    &mut chunk_worker,
                    &mut stats_counter,
                    &id_maps,
                    &mut input_state_slot,
                )
                .await?
                {
                    break;
                }
                // 插件动作帧立即排空（本包事件触发的跨玩家下发）
                drain_plugin_frames(&mut frame_rx, sink).await?;
                sink.flush().await?;
            }
            Some(frame) = frame_rx.recv() => {
                // 插件触发的动作帧（其他时机入队的）写回客户端
                sink.send_encoded_frame(frame).await?;
                drain_plugin_frames(&mut frame_rx, sink).await?;
                sink.flush().await?;
            }
            Some(job) = chunk_worker.recv_done() => {
                // worker 完成一批：主循环只做下发（纯内存写），不再等网络/光照。
                // 出生批与增量批同构（ChunkBatch 包裹），只差日志口径与统计。
                let Some(chunk_worker) = chunk_worker.as_mut() else {
                    continue;
                };
                for &(x, z) in &job.coords {
                    chunk_worker.complete(x, z);
                }
                if job.spawn_batch {
                    stats.chunks_sent += job.coords.len() as u64;
                }
                match job.result {
                    Ok(chunks) => {
                        qexed_world::stream::send_chunk_batch(sink, &chunks)
                            .await
                            .map_err(|e| PlayError::ChunkTcp(e.to_string()))?;
                        for chunk in &chunks {
                            streaming.sent.insert((chunk.chunk_x, chunk.chunk_z));
                        }
                        if job.spawn_batch {
                            log::info!(
                                "出生批次下发 {} 块（累计 {}）",
                                chunks.len(),
                                stats.chunks_sent
                            );
                        } else {
                            stats_log(chunks.len(), (view_center.0, view_center.1));
                        }
                    }
                    Err(err) if play_config.chunk_tcp_fallback_empty => {
                        log::warn!(
                            "{}区块请求失败（{} 个），回退空区块: {err}",
                            if job.spawn_batch { "出生" } else { "增量" },
                            job.coords.len()
                        );
                        let chunks: Vec<_> = job
                            .coords
                            .iter()
                            .map(|&(x, z)| qexed_world::chunk::empty_chunk_packet(x, z))
                            .collect();
                        qexed_world::stream::send_chunk_batch(sink, &chunks)
                            .await
                            .map_err(|e| PlayError::ChunkTcp(e.to_string()))?;
                        for &(x, z) in &job.coords {
                            streaming.sent.insert((x, z));
                        }
                    }
                    Err(err) => {
                        log::warn!(
                            "{}区块请求失败（{} 个），本轮跳过: {err}",
                            if job.spawn_batch { "出生" } else { "增量" },
                            job.coords.len()
                        );
                    }
                }
                sink.flush().await?;
            }
            _ = ai_tick.tick() => {
                drive_entity_ai(&player).await;
            }
        }
    }

    // 退服事件后仍可能有队尾帧，最后排一次
    drain_plugin_frames(&mut frame_rx, sink).await?;

    // ── 插件事件：退服 ──
    fire_plugin_event(&qexed_plugin_api::PlayerLeaveEvent::new(plugin_player_info(&player)))
        .await;

    // ── 插件桥：注销出站帧队列（leave handler 仍可向本玩家下发，见上排空） ──
    hub.unregister_player_session(&player.profile.username);

    stats.duration_secs = started.elapsed().as_secs();
    log::info!(
        "player {} left play phase ({} packets in {}s)",
        player.display_name(),
        stats.packets_received,
        stats.duration_secs
    );
    Ok(stats)
}

/// 解码结果：packet id + 已解码的包体（未知包保留 id 供日志）。
#[derive(Debug)]
enum Decoded {
    KeepAlive(play::keep_alive::KeepAlive),
    ChatSessionUpdate(play::chat_session_update::ChatSessionUpdate),
    ChatAck(play::chat_ack::ChatAck),
    ChatMessage(play::chat_message::ChatMessage),
    ChatCommand(play::chat_command::ChatCommand),
    MovePos(play::move_player_pos::MovePlayerPos),
    MovePosRot(play::move_player_pos_rot::MovePlayerPosRot),
    MoveRot(play::move_player_rot::MovePlayerRot),
    MoveStatus,
    Attack(play::attack::Attack),
    Interact(play::interact::Interact),
    PlayerAction(play::player_action::PlayerAction),
    PlayerInput(play::player_input::PlayerInput),
    UseItem(play::use_item::UseItem),
    UseItemOn(play::use_item_on::UseItemOn),
    Abilities(play::player_abilities::PlayerAbilities),
    ChunkBatch(play::chunk_batch_received::ChunkBatchReceived),
    ClientCommand(play::client_command::ClientCommand),
    AcceptTeleportation,
    PlayerLoaded,
    Pong,
    Unknown(i32),
}

/// 同步解码一个 serverbound 包。
fn decode(payload: &mut bytes::BytesMut) -> Decoded {
    let mut reader = PacketReader::new(payload);
    let Ok(id) = reader.deserialize::<VarInt>() else {
        return Decoded::Unknown(-1);
    };
    macro_rules! try_decode {
        ($ty:ty, $variant:ident) => {
            if id.0 == <$ty as qexed_packet::Packet>::ID {
                let mut p = <$ty as Default>::default();
                if p.deserialize(&mut reader).is_ok() {
                    return Decoded::$variant(p);
                }
                return Decoded::Unknown(id.0);
            }
        };
    }

    try_decode!(play::keep_alive::KeepAlive, KeepAlive);
    try_decode!(
        play::chat_session_update::ChatSessionUpdate,
        ChatSessionUpdate
    );
    try_decode!(play::chat_ack::ChatAck, ChatAck);
    try_decode!(play::chat_message::ChatMessage, ChatMessage);
    try_decode!(play::chat_command::ChatCommand, ChatCommand);
    try_decode!(play::move_player_pos::MovePlayerPos, MovePos);
    try_decode!(play::move_player_pos_rot::MovePlayerPosRot, MovePosRot);
    try_decode!(play::move_player_rot::MovePlayerRot, MoveRot);
    try_decode!(play::player_abilities::PlayerAbilities, Abilities);
    try_decode!(play::chunk_batch_received::ChunkBatchReceived, ChunkBatch);
    try_decode!(play::client_command::ClientCommand, ClientCommand);
    try_decode!(play::attack::Attack, Attack);
    try_decode!(play::interact::Interact, Interact);
    try_decode!(play::player_action::PlayerAction, PlayerAction);
    try_decode!(play::player_input::PlayerInput, PlayerInput);
    try_decode!(play::use_item::UseItem, UseItem);
    try_decode!(play::use_item_on::UseItemOn, UseItemOn);

    // 无包体的固定 id 包
    if id.0 == play::move_player_status_only::MovePlayerStatusOnly::ID {
        // 仅 flags，无有用数据；解码失败也按已知包处理
        return Decoded::MoveStatus;
    }
    if id.0 == play::accept_teleportation::AcceptTeleportation::ID {
        return Decoded::AcceptTeleportation;
    }
    if id.0 == play::player_loaded::PlayerLoaded::ID {
        return Decoded::PlayerLoaded;
    }
    if id.0 == play::pong::Pong::ID {
        return Decoded::Pong;
    }

    Decoded::Unknown(id.0)
}

/// 按解码结果执行行为。返回 `true` 表示请求结束会话。
#[allow(clippy::too_many_arguments)]
async fn apply<W>(
    player: &mut PlayPlayer,
    chat: &mut qexed_chat::ChatMessageHandler,
    commands: &mut qexed_command::CommandTree,
    command_effects: &Arc<Mutex<Vec<CommandEffect>>>,
    scoreboard: &Arc<Mutex<qexed_scoreboard::Scoreboard>>,
    scoreboard_ops: &qexed_scoreboard::command::OpQueue,
    decoded: Decoded,
    sink: &mut PacketSink<W>,
    stats: &mut session::PlaySessionStats,
    pending_keepalive: &mut Option<(i64, Instant)>,
    view_center: &mut (i32, i32),
    streaming: &mut session::ChunkStreamingState,
    chunk_worker: &mut chunk_worker::SessionWorker,
    stats_counter: &mut StatCounter,
    id_maps: &Option<IdMaps>,
    input_state: &mut qexed_plugin_api::PlayerInputState,
) -> Result<bool, PlayError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    match decoded {
        // ── 心跳回应 ──
        Decoded::KeepAlive(ka) => {
            if matches!(pending_keepalive, Some((id, _)) if *id == ka.keep_alive_id) {
                *pending_keepalive = None;
                log::debug!("play keepalive ack id={}", ka.keep_alive_id);
            } else {
                log::debug!(
                    "stale keepalive ack id={} (expected {:?})",
                    ka.keep_alive_id,
                    pending_keepalive.as_ref().map(|(id, _)| *id)
                );
            }
        }
        // ── 聊天：行为在 qexed_chat ──
        Decoded::ChatSessionUpdate(p) => {
            chat.update_session(&p.chat_session)?;
            log::debug!("chat session updated: {}", player.display_name());
            // 对齐 vanilla：会话上报后广播 INITIALIZE_CHAT，让（包括发送者在内的）
            // 所有客户端建立 RemoteChatSession。没有这一步，客户端会用
            // unsigned 链重验我们的签名转发，必然弹出 "chat.validation_error"。
            sink.send(to_client::play::player_info_update::PlayerInfoUpdate {
                actions: to_client::play::player_info_update::PlayerInfoActions::player_initializing(),
                entries: vec![{
                    let mut entry = to_client::play::player_info_update::PlayerInfoEntry::from_profile(
                        &player.profile,
                        player.game_mode as i32,
                    );
                    entry.chat_session = Some(p.chat_session.clone());
                    entry
                }],
            })
            .await?;
        }
        Decoded::ChatAck(p) => {
            chat.apply_offset(p.offset)?;
        }
        Decoded::ChatMessage(p) => {
            // jev 门控：内容检测（Block 拦截 / Sanitize 打码）+ 村长 NPC 触发。
            // 未启用 AI 时退化为原样放行，等价于直通 qexed_chat。
            let outcome = jev_bridge::gate_chat_message(player.profile.uuid, &p.message).await;
            let reply = outcome.reply().map(str::to_string);
            match outcome {
                JevChatOutcome::Blocked { notice } => {
                    send_system(sink, notice).await?;
                }
                // 打码照转（签名链要求），发送者额外收到拦截提示：
                // 玩家视角=消息没发出去；其他人看到打码文本。
                JevChatOutcome::Sanitized { text, notice } => {
                    let chat_event =
                        qexed_plugin_api::PlayerChatEvent::new(plugin_player_info(&player), text.clone());
                    let result = fire_plugin_event(&chat_event).await;
                    if result.is_cancelled() {
                        send_system(sink, notice).await?;
                        return Ok(false); // 继续会话
                    }
                    match chat.handle_unfiltered_with(&player.profile, &p, text) {
                        Ok(accepted) => {
                            stats.chat_messages += 1;
                            sink.send_raw(accepted.packet).await?;
                            send_system(sink, notice).await?;
                        }
                        Err(err) => {
                            log::warn!("sanitized chat rejected: {err}");
                            send_system(sink, notice).await?;
                        }
                    }
                }
                JevChatOutcome::Allowed { text } | JevChatOutcome::AllowedWithReply { text, .. } => {
                    // jev 已在桥里定稿广播文本（放行原文或打码文本）。
                    // 这里无过滤语义：内容被改写时 handle 内部走原签名转发 +
                    // unsigned_chat_content 覆盖显示，签名链保持连续不报错。
                    // 插件事件：玩家聊天；任一 handler cancel → 丢弃本条消息
                    let chat_event = qexed_plugin_api::PlayerChatEvent::new(
                        plugin_player_info(&player),
                        text.clone(),
                    );
                    let result = fire_plugin_event(&chat_event).await;
                    let chat_blocked = result.is_cancelled();
                    if !chat_blocked {
                        match chat.handle_unfiltered_with(&player.profile, &p, text) {
                            Ok(accepted) => {
                                stats.chat_messages += 1;
                                // 单人服：广播 = 发回自己；多玩家广播由后续 PlayerManager 接管
                                sink.send_raw(accepted.packet).await?;
                            if let Some(reply) = reply {
                                send_system(sink, format!("村长：{reply}")).await?;
                            }
                        }
                            Err(qexed_chat::ChatError::RateLimited) => {
                                send_system(sink, "You are sending messages too fast.").await?;
                            }
                            Err(qexed_chat::ChatError::MessageTooLong { length, max_length }) => {
                                send_system(
                                    sink,
                                    format!("Message too long ({length} > {max_length})."),
                                )
                                .await?;
                            }
                            Err(err) => {
                                log::warn!("chat rejected: {err}");
                                send_system(sink, "Message could not be sent.").await?;
                            }
                        }
                    }
                }
            }
        }
        Decoded::ChatCommand(p) => {
            log::info!("command from {}: /{}", player.display_name(), p.command);
            // 委托 qexed_command 解析分发；失败语义转系统聊天回显
            let input = p.command.trim_start_matches('/');
            // ── 插件命令路由：顶层命令命中注册表 → ChatCommandEvent ──
            // cancel = 插件接管（v4 commands 事件语义）；未取消则继续内置树。
            // 'packet：异步循环体内不能 continue，用标记块跳过本包后续处理。
            'packet: {
                let (head, rest) = match input.split_once(' ') {
                    Some((h, r)) => (h, r),
                    None => (input, ""),
                };
                let is_plugin_command =
                    qexed_plugin::action_hub().plugin_command_names().iter().any(|c| c == head);
                if is_plugin_command {
                    let result = fire_plugin_event(&qexed_plugin_api::ChatCommandEvent::new(
                        plugin_player_info(player),
                        head,
                        rest,
                    ))
                    .await;
                    if result.is_cancelled() {
                        stats.commands_executed += 1;
                        break 'packet;
                    }
                }
            }
            let _cmd_span = qexed_profiler::profile_span!("cmd:dispatch");
            match commands.dispatch(input) {
                Ok(()) => {
                    stats.commands_executed += 1;
                    // executor 上报的副作用统一在此代发（树不持有 sink）。
                    // 先收集再处理：MutexGuard 不得存活跨 await（否则 future 非 Send）
                    // /scoreboard：先应用 op（锁内同步完成），转成同步包副作用
                    let ops: Vec<qexed_scoreboard::command::ScoreboardOp> =
                        scoreboard_ops.lock().unwrap().drain(..).collect();
                    let mut sb_effects: Vec<CommandEffect> = Vec::new();
                    if !ops.is_empty() {
                        let mut sb = scoreboard.lock().unwrap();
                        for op in ops {
                            for fx in qexed_scoreboard::command::apply_op(&mut sb, op) {
                                sb_effects.push(match fx {
                                    qexed_scoreboard::command::ScoreboardEffect::Reply(text) => {
                                        CommandEffect::Reply(text)
                                    }
                                    qexed_scoreboard::command::ScoreboardEffect::Packet(p) => {
                                        CommandEffect::ScoreboardPacket(p)
                                    }
                                    qexed_scoreboard::command::ScoreboardEffect::Rejected(msg) => {
                                        CommandEffect::ScoreboardRejected(msg)
                                    }
                                });
                            }
                        }
                    }
                    let mut effects: Vec<CommandEffect> =
                        command_effects.lock().unwrap().drain(..).collect();
                    effects.extend(sb_effects);
                    for effect in effects {
                        match effect {
                            CommandEffect::Reply(text) => send_system(sink, text).await?,
                            CommandEffect::SetGameMode(mode) => {
                                player.game_mode = mode;
                                sink.send(to_client::play::game_state_change::GameStateChange {
                                    reason: 3, // change_game_mode
                                    game_mode: mode as f32,
                                })
                                .await?;
                            }
                            CommandEffect::ScoreboardPacket(packet) => {
                                match packet {
                                    qexed_scoreboard::SyncPacket::Objective(p) => sink.send(p).await?,
                                    qexed_scoreboard::SyncPacket::Score(p) => sink.send(p).await?,
                                    qexed_scoreboard::SyncPacket::Reset(p) => sink.send(p).await?,
                                    qexed_scoreboard::SyncPacket::Display(p) => sink.send(p).await?,
                                }
                            }
                            CommandEffect::ScoreboardRejected(msg) => send_system(sink, msg).await?,
                        }
                    }
                }
                Err(CommandError::UnknownCommand(name)) => {
                    let hint = if name.is_empty() {
                        String::new()
                    } else {
                        format!("Unknown command: /{name}")
                    };
                    send_system(sink, hint).await?;
                }
                Err(CommandError::InvalidUsage { usage }) => {
                    send_system(sink, format!("Usage: /{usage}")).await?;
                }
                Err(CommandError::BadArgument { name, expected, raw }) => {
                    send_system(
                        sink,
                        format!("Incorrect argument for \"{name}\": expected {expected}, got \"{raw}\""),
                    )
                    .await?;
                }
                Err(CommandError::Custom(msg)) => {
                    send_system(sink, msg).await?;
                }
            }
        }
        // ── 移动：插件事件（cancel = 不落地）→ 更新会话状态；跨区块增量流送 ──
        Decoded::MovePos(p) => {
            if emit_move_event(player, Some((p.x, p.y, p.z)), None, input_state).await {
                enqueue_view_streaming(
                    sink,
                    &player,
                    view_center,
                    streaming,
                    chunk_worker,
                )
                .await?;
            }
        }
        Decoded::MovePosRot(p) => {
            if emit_move_event(player, Some((p.x, p.y, p.z)), Some((p.yaw, p.pitch)), input_state).await {
                enqueue_view_streaming(
                    sink,
                    &player,
                    view_center,
                    streaming,
                    chunk_worker,
                )
                .await?;
            }
        }
        Decoded::MoveRot(p) => {
            if emit_move_event(player, None, Some((p.yaw, p.pitch)), input_state).await {
                // 纯视角变化不跨区块，无需视点同步
            }
        }
        Decoded::MoveStatus => {
            if emit_move_event(player, None, None, input_state).await {
                // on_ground 翻转也可能影响插件寻路；宿主无感知变更
            }
        }
        // ── 交互：攻击 / 实体交互 / 方块动作 / 输入 / 用物 ──
        Decoded::Attack(p) => {
            emit_attack_event(player, p.entity_id.0).await;
        }
        Decoded::Interact(p) => {
            // 手：0=主手 1=副手；attack(2/无 location) 走 NpcInteract 的 attack 语义
            if p.hand.0 == 2 {
                emit_npc_interact(player, p.entity_id.0, "attack", "").await;
            } else {
                let hand = if p.hand.0 == 1 { "off_hand" } else { "main_hand" };
                emit_npc_interact(player, p.entity_id.0, "interact", hand).await;
            }
        }
        Decoded::PlayerAction(p) => {
            handle_player_action(player, &p, input_state).await;
        }
        Decoded::PlayerInput(p) => {
            let next = input_state_from_flags(p.flags);
            if next != *input_state {
                let previous = *input_state;
                *input_state = next;
                fire_plugin_event(&qexed_plugin_api::PlayerInputEvent {
                    player: plugin_player_info(player),
                    dimension: player.config.dimension_name.clone(),
                    position: player_position_payload(player),
                    previous_input: previous,
                    input: next,
                })
                .await;
            }
        }
        Decoded::UseItem(p) => {
            emit_use_item_event(player, &p).await;
        }
        Decoded::UseItemOn(p) => {
            emit_block_interact_event(player, &p, input_state).await;
        }
        // ── 传送确认 / 加载完成 / ping / 批次 ──
        Decoded::AcceptTeleportation => {
            log::debug!("teleport confirmed");
        }
        Decoded::PlayerLoaded => {
            player.loaded = true;
            input_state.on_ground = true;
            log::debug!("player loaded");
        }
        Decoded::Pong => {
            log::debug!("play pong");
        }
        Decoded::ChunkBatch(p) => {
            log::debug!(
                "chunk batch received, desired chunks/tick={:.2}",
                p.desired_chunks_per_tick
            );
        }
        // ── 统计屏幕：客户端请求全量 dirty 集（原版 REQUEST_STATS 唯一推包点） ──
        Decoded::ClientCommand(p) if p.action.0 == play::client_command::REQUEST_STATS => {
            if let Some(maps) = id_maps {
                if let Some(packet) = stats_counter.take_award_stats(maps) {
                    sink.send(packet).await?;
                    log::debug!("sent award_stats to {}", player.display_name());
                }
            }
        }
        Decoded::ClientCommand(_) => {
            log::debug!("client command (respawn/game rule) ignored");
        }
        // ── 客户端能力开关 ──
        Decoded::Abilities(p) => {
            log::debug!("abilities flags=0x{:02x}", p.flags);
        }
        // ── 其余：记录并忽略，行为随对应 crate 落地逐步接入 ──
        Decoded::Unknown(id) => {
            log::debug!(
                "unhandled play packet id=0x{:02x} from {}",
                id,
                player.display_name()
            );
        }
    }
    Ok(false)
}

/// 玩家跨区块后的增量视点同步（Pumpkin `chunker::update_position` 的跑图版）：
///
/// 1. UpdateViewPosition 通知客户端新视点；
/// 2. 视野圈差分：新圈增量（按近到远）经 TcpChunkSource 批量取包，
///    ChunkBatchStart/Finished 包裹下发；旧圈增量逐个 ForgetLevelChunk；
/// 3. sent 集合与视野圈同步收敛。
///
/// TcpChunkSource 缺席（empty 源 / 连接失败）时只发 UpdateViewPosition；
/// 增量请求失败降级空区块（`chunk_tcp_fallback_empty`）或跳过本次加载，
/// 不中断 PLAY 主循环——跑图丢一环保玩整个会话。
async fn enqueue_view_streaming<W>(
    sink: &mut PacketSink<W>,
    player: &PlayPlayer,
    view_center: &mut (i32, i32),
    streaming: &mut session::ChunkStreamingState,
    chunk_worker: &mut chunk_worker::SessionWorker,
) -> Result<(), PlayError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let now = player.chunk_pos();
    if now == *view_center {
        return Ok(());
    }
    *view_center = now;
    sink.send(to_client::play::update_view_position::UpdateViewPosition {
        chunk_x: VarInt(now.0),
        chunk_z: VarInt(now.1),
    })
    .await?;
    let Some(worker) = chunk_worker.as_mut() else {
        return Ok(());
    };

    // 视野圈推进 + 差分（先把状态落定，失败只影响本轮加载集）。
    let moved = qexed_world::stream::ViewCircle::new(now, streaming.circle.radius());
    let gained = qexed_world::stream::ViewCircle::gained(&streaming.circle, &moved);
    let lost = qexed_world::stream::ViewCircle::lost(&streaming.circle, &moved);
    streaming.circle = moved;

    // 卸载：旧圈超出新圈的部分逐个 Forget。
    for (x, z) in &lost {
        sink.send(ForgetLevelChunk {
            chunk_x: *x,
            chunk_z: *z,
        })
        .await?;
        streaming.sent.remove(&(*x, *z));
    }

    // 加载：批量请求入 worker（在途去重），完成品由主循环 select 分支下发。
    // 本函数全程无网络/光照 await——这是移动包不再堵塞主循环的关键。
    let _incr_span = qexed_profiler::profile_span!("world:incremental_stream");
    for batch in gained.chunks(qexed_world::stream::CHUNKS_PER_BATCH) {
        // 全程无网络/光照 await,入队纯内存(unbounded)——移动包处理零阻塞。
        worker.enqueue(batch.to_vec());
    }
    Ok(())
}

fn stats_log(count: usize, center: (i32, i32)) {
    log::debug!("incremental chunks streamed: {} @ ({},{})", count, center.0, center.1);
}

async fn send_system<W>(sink: &mut PacketSink<W>, text: impl Into<String>) -> Result<(), PlayError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(to_client::play::system_chat::SystemChat {
        content: qexed_chat::text_component(text),
        overlay: false,
    })
    .await?;
    Ok(())
}
// ---------------------------------------------------------------------------
// 插件事件辅助：构造载荷 → fire（无插件时零开销）
// ---------------------------------------------------------------------------

fn player_position_payload(player: &PlayPlayer) -> qexed_plugin_api::PlayerPositionPayload {
    qexed_plugin_api::PlayerPositionPayload::from_parts(
        player.position.0,
        player.position.1,
        player.position.2,
        player.rotation.0,
        player.rotation.1,
        true,
    )
}

/// 移动事件：cancel = 位置/视角更新不落地。返回事件是否放行。
async fn emit_move_event(
    player: &mut PlayPlayer,
    position: Option<(f64, f64, f64)>,
    rotation: Option<(f32, f32)>,
    input_state: &qexed_plugin_api::PlayerInputState,
) -> bool {
    let previous = qexed_plugin_api::PlayerPositionPayload::from_parts(
        player.position.0,
        player.position.1,
        player.position.2,
        player.rotation.0,
        player.rotation.1,
        input_state.on_ground,
    );
    let (nx, ny, nz) = position.unwrap_or(player.position);
    let (nyaw, npitch) = rotation.unwrap_or(player.rotation);
    let event = qexed_plugin_api::PlayerMoveEvent {
        player: plugin_player_info(player),
        dimension: player.config.dimension_name.clone(),
        previous,
        position: qexed_plugin_api::PlayerPositionPayload::from_parts(
            nx, ny, nz, nyaw, npitch, input_state.on_ground,
        ),
    };
    let result = fire_plugin_event(&event).await;
    if result.is_cancelled() {
        return false;
    }
    player.update_position(position, rotation);
    true
}

/// 攻击事件 + NPC 攻击路由（无登记实体时退化为普通 Attack 事件）。
async fn emit_attack_event(player: &PlayPlayer, target_entity_id: i32) {
    if emit_npc_interact(player, target_entity_id, "attack", "").await {
        return;
    }
    fire_plugin_event(&qexed_plugin_api::PlayerAttackEvent {
        player: plugin_player_info(player),
        target_entity_id,
        damage: 1.0,
        critical: false,
    })
    .await;
}

/// NPC 交互路由：按实体 id 查 ActionHub 登记表，命中才发 NpcInteractEvent。
/// 返回是否命中（未命中调用方继续普通事件路径）。
async fn emit_npc_interact(
    player: &PlayPlayer,
    entity_id: i32,
    action: &str,
    hand: &str,
) -> bool {
    let interaction = if hand == "off_hand" {
        qexed_plugin::host_api::action::NpcInteraction::OffHand
    } else if action == "attack" {
        qexed_plugin::host_api::action::NpcInteraction::Attack
    } else {
        qexed_plugin::host_api::action::NpcInteraction::MainHand
    };
    let Some((key, spec, configured_event)) =
        qexed_plugin::action_hub().npc_event_by_entity_id(entity_id, interaction)
    else {
        return false;
    };
    fire_plugin_event(&qexed_plugin_api::NpcInteractEvent {
        payload: qexed_plugin_api::NpcInteractPayload {
            player: qexed_plugin_api::PlayerPayloadOwned::from_info(&plugin_player_info(player)),
            entity: qexed_plugin_api::NpcEntityPayload {
                key,
                entity_id,
                dimension: spec.dimension,
                x: spec.x,
                y: spec.y,
                z: spec.z,
                yaw: spec.yaw,
                pitch: spec.pitch,
            },
            action: action.to_string(),
            hand: hand.to_string(),
            configured_event,
        },
    })
    .await;
    true
}

/// 方块动作：START_DIGGING 触发方块事件（近似 v4 block_step）；FINISHED_DIGGING
/// 语义简化——v6 首批无方块权威读取，瞬时破坏下 START 即完成。
async fn handle_player_action(
    player: &PlayPlayer,
    p: &play::player_action::PlayerAction,
    input_state: &qexed_plugin_api::PlayerInputState,
) {
    const START_DIGGING: i32 = 0;
    const FINISHED_DIGGING: i32 = 2;
    match p.status.0 {
        START_DIGGING => {
            fire_plugin_event(&qexed_plugin_api::PlayerBlockStepEvent {
                player: plugin_player_info(player),
                dimension: player.config.dimension_name.clone(),
                position: qexed_plugin_api::PlayerPositionPayload::from_parts(
                    player.position.0,
                    player.position.1,
                    player.position.2,
                    player.rotation.0,
                    player.rotation.1,
                    input_state.on_ground,
                ),
                block: qexed_plugin_api::BlockStepPosition {
                    x: p.location.x,
                    y: p.location.y,
                    z: p.location.z,
                },
            })
            .await;
        }
        FINISHED_DIGGING => {
            // 客户端预测破坏等 sequence 确认；无方块权威时无内建动作
        }
        _ => {}
    }
}

fn input_state_from_flags(flags: u8) -> qexed_plugin_api::PlayerInputState {
    use qexed_protocol::to_server::play::player_input as pi;
    qexed_plugin_api::PlayerInputState {
        forward: flags & pi::FORWARD != 0,
        backward: flags & pi::BACKWARD != 0,
        left: flags & pi::LEFT != 0,
        right: flags & pi::RIGHT != 0,
        jump: flags & pi::JUMP != 0,
        shift: flags & pi::SHIFT != 0,
        sprint: flags & pi::SPRINT != 0,
        on_ground: false,
    }
}

/// 玩家使用物品（未指向方块）。v6 首批无背包读取，物品载荷为空占位。
async fn emit_use_item_event(player: &PlayPlayer, p: &play::use_item::UseItem) {
    fire_plugin_event(&qexed_plugin_api::PlayerUseItemEvent {
        player: plugin_player_info(player),
        dimension: player.config.dimension_name.clone(),
        position: player_position_payload(player),
        hand: if p.hand.0 == 1 { "off_hand" } else { "main_hand" }.to_string(),
        action: "use".to_string(),
        item: qexed_plugin_api::ItemStackPayload::default(),
        sequence: p.sequence.0,
        yaw: p.yaw,
        pitch: p.pitch,
    })
    .await;
}

/// 方块交互（UseItemOn）：interact 置位 = 插件接管（宿主无内建方块行为，占位）。
async fn emit_block_interact_event(
    player: &PlayPlayer,
    p: &play::use_item_on::UseItemOn,
    input_state: &qexed_plugin_api::PlayerInputState,
) {
    const FACE_NAMES: [&str; 7] = ["bottom", "top", "north", "south", "west", "east", "unknown"];
    fire_plugin_event(&qexed_plugin_api::PlayerBlockInteractEvent {
        player: plugin_player_info(player),
        dimension: player.config.dimension_name.clone(),
        position: player_position_payload(player),
        block: qexed_plugin_api::BlockDropPosition {
            x: p.block_hit.position.x,
            y: p.block_hit.position.y,
            z: p.block_hit.position.z,
        },
        block_state: 0,
        block_name: String::new(),
        hand: if p.hand.0 == 1 { "off_hand" } else { "main_hand" }.to_string(),
        sequence: p.sequence.0,
        face: FACE_NAMES
            .get(p.block_hit.face.0 as usize)
            .copied()
            .unwrap_or("unknown")
            .to_string(),
        face_id: p.block_hit.face.0,
        cursor_x: p.block_hit.cursor_x,
        cursor_y: p.block_hit.cursor_y,
        cursor_z: p.block_hit.cursor_z,
        inside_block: p.block_hit.inside_block,
        world_border_hit: p.block_hit.world_border_hit,
        input: *input_state,
        interact: false,
    })
    .await;
}

/// 排空插件帧队列写入 sink。
async fn drain_plugin_frames<W>(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<qexed_plugin::host_api::action::OutboundFrame>,
    sink: &mut PacketSink<W>,
) -> Result<(), PlayError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    while let Ok(frame) = rx.try_recv() {
        sink.send_encoded_frame(frame).await?;
    }
    Ok(())
}

/// 实体 AI 节拍：逐实体组装查询 → fire → 收集操作下发。
async fn drive_entity_ai(player: &PlayPlayer) {
    let hub = qexed_plugin::action_hub();
    let keys = hub.ai_entity_keys();
    if keys.is_empty() {
        return;
    }
    let _ = player; // 单服单玩家：本会话玩家即 nearby 邻居（首批不聚合）
    for key in keys {
        let Some(query) = hub.ai_tick_query(&key, AI_TICK_INTERVAL_MS, &[]) else {
            continue;
        };
        let event = qexed_plugin_api::EntityAiTickEvent {
            query,
            response: std::sync::Mutex::new(qexed_plugin_api::EntityAiTickResponse::default()),
        };
        fire_plugin_event(&event).await;
        let ops = event.response.into_inner().unwrap().operations;
        if !ops.is_empty() {
            hub.apply_ai_operations(&key, &ops);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::{PacketCodec as _, PacketWriter};

    fn encode_id(id: i32) -> bytes::BytesMut {
        let mut buf = bytes::BytesMut::new();
        let mut w = PacketWriter::new(&mut buf);
        VarInt(id).serialize(&mut w).expect("serialize varint id");
        buf
    }

    fn encode_full<T: qexed_packet::Packet>(p: &T) -> bytes::BytesMut {
        let mut buf = bytes::BytesMut::new();
        let mut w = PacketWriter::new(&mut buf);
        VarInt(<T as qexed_packet::Packet>::ID)
            .serialize(&mut w)
            .expect("serialize id");
        p.serialize(&mut w).expect("serialize body");
        buf
    }

    #[test]
    fn decode_known_ids_without_body() {
        let mut payload = encode_id(play::accept_teleportation::AcceptTeleportation::ID);
        assert!(matches!(decode(&mut payload), Decoded::AcceptTeleportation));

        let mut payload = encode_id(play::player_loaded::PlayerLoaded::ID);
        assert!(matches!(decode(&mut payload), Decoded::PlayerLoaded));

        let mut payload = encode_id(play::pong::Pong::ID);
        assert!(matches!(decode(&mut payload), Decoded::Pong));

        let mut payload = encode_id(play::move_player_status_only::MovePlayerStatusOnly::ID);
        assert!(matches!(decode(&mut payload), Decoded::MoveStatus));
    }

    #[test]
    fn decode_keepalive_roundtrip() {
        let ka = play::keep_alive::KeepAlive { keep_alive_id: 42 };
        let mut payload = encode_full(&ka);
        match decode(&mut payload) {
            Decoded::KeepAlive(decoded) => assert_eq!(decoded.keep_alive_id, 42),
            other => panic!("expected KeepAlive, got {other:?}"),
        }
    }

    #[test]
    fn decode_move_pos_rot() {
        let p = play::move_player_pos_rot::MovePlayerPosRot {
            x: 1.5,
            y: 64.0,
            z: -3.25,
            yaw: 90.0,
            pitch: -10.0,
            flags: 0,
        };
        let mut payload = encode_full(&p);
        match decode(&mut payload) {
            Decoded::MovePosRot(d) => {
                assert_eq!((d.x, d.y, d.z), (1.5, 64.0, -3.25));
                assert_eq!((d.yaw, d.pitch), (90.0, -10.0));
            }
            other => panic!("expected MovePosRot, got {other:?}"),
        }
    }

    #[test]
    fn decode_unknown_id() {
        let mut payload = encode_id(0x7f);
        match decode(&mut payload) {
            Decoded::Unknown(id) => assert_eq!(id, 0x7f),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn decode_empty_payload() {
        let mut payload = bytes::BytesMut::new();
        assert!(matches!(decode(&mut payload), Decoded::Unknown(-1)));
    }

    /// 构造默认树 + 空记分板，dispatch 后返回 drain 出的全部副作用。
    fn dispatch_line(
        commands: &mut qexed_command::CommandTree,
        scoreboard: &Arc<Mutex<qexed_scoreboard::Scoreboard>>,
        scoreboard_ops: &qexed_scoreboard::command::OpQueue,
        command_effects: &Arc<Mutex<Vec<CommandEffect>>>,
        line: &str,
    ) -> Vec<CommandEffect> {
        commands.dispatch(line).expect("dispatch ok");
        let mut effects: Vec<CommandEffect> = Vec::new();
        let ops: Vec<qexed_scoreboard::command::ScoreboardOp> =
            scoreboard_ops.lock().unwrap().drain(..).collect();
        {
            let mut sb = scoreboard.lock().unwrap();
            for op in ops {
                for fx in qexed_scoreboard::command::apply_op(&mut sb, op) {
                    effects.push(match fx {
                        qexed_scoreboard::command::ScoreboardEffect::Reply(text) => {
                            CommandEffect::Reply(text)
                        }
                        qexed_scoreboard::command::ScoreboardEffect::Packet(p) => {
                            CommandEffect::ScoreboardPacket(p)
                        }
                        qexed_scoreboard::command::ScoreboardEffect::Rejected(msg) => {
                            CommandEffect::ScoreboardRejected(msg)
                        }
                    });
                }
            }
        }
        effects.extend(command_effects.lock().unwrap().drain(..));
        effects
    }

    #[test]
    fn help_lists_all_commands() {
        let fx_q = Arc::new(Mutex::new(Vec::new()));
        let sb = Arc::new(Mutex::new(qexed_scoreboard::Scoreboard::new()));
        let ops_q: qexed_scoreboard::command::OpQueue = Arc::new(Mutex::new(Vec::new()));
        let mut tree = bootstrap::default_command_tree(fx_q.clone(), ops_q.clone());

        let fx = dispatch_line(&mut tree, &sb, &ops_q, &fx_q, "help");
        assert_eq!(fx.len(), 1);
        let CommandEffect::Reply(msg) = &fx[0] else {
            panic!("expected Reply");
        };
        for name in ["/ping", "/gamemode", "/say", "/scoreboard", "/help"] {
            assert!(msg.contains(name), "help missing {name}: {msg}");
        }
    }

    #[test]
    fn scoreboard_command_updates_state_and_emits_packets() {
        let fx_q = Arc::new(Mutex::new(Vec::new()));
        let sb = Arc::new(Mutex::new(qexed_scoreboard::Scoreboard::new()));
        let ops_q: qexed_scoreboard::command::OpQueue = Arc::new(Mutex::new(Vec::new()));
        let mut tree = bootstrap::default_command_tree(fx_q.clone(), ops_q.clone());

        // 建目标：op → 状态 + SetObjective 同步包 + 回显
        let fx = dispatch_line(&mut tree, &sb, &ops_q, &fx_q, "scoreboard objectives add kills dummy Kills");
        assert!(matches!(&fx[0], CommandEffect::ScoreboardPacket(qexed_scoreboard::SyncPacket::Objective(_))));
        assert!(matches!(&fx[1], CommandEffect::Reply(m) if m.contains("kills")));
        assert!(sb.lock().unwrap().has_objective("kills"));

        // 记分：SetScore 包 + 状态落库
        let fx = dispatch_line(&mut tree, &sb, &ops_q, &fx_q, "scoreboard players set alice kills 5");
        assert!(matches!(&fx[0], CommandEffect::ScoreboardPacket(qexed_scoreboard::SyncPacket::Score(_))));
        assert_eq!(sb.lock().unwrap().get_score("kills", "alice"), Some(5));

        // 未知目标：拒绝转错误回显，不发数据包
        let fx = dispatch_line(&mut tree, &sb, &ops_q, &fx_q, "scoreboard players set bob ghost 1");
        assert!(matches!(&fx[0], CommandEffect::ScoreboardRejected(m) if m.contains("ghost")));

        // 未知指令仍走原有错误路径
        assert!(tree.dispatch("scoreboard nonsense").is_err());
    }

    #[test]
    fn declare_commands_packet_contains_scoreboard_tree() {
        let fx_q = Arc::new(Mutex::new(Vec::new()));
        let ops_q: qexed_scoreboard::command::OpQueue = Arc::new(Mutex::new(Vec::new()));
        let tree = bootstrap::default_command_tree(fx_q, ops_q);

        // 序列化成功且节点数多于加 /scoreboard 之前（原树 ping/gamemode/say/help = 10 节点）
        let packet = bootstrap::command_tree_packet(&tree);
        assert!(packet.nodes.len() > 10, "nodes = {}", packet.nodes.len());
    }
}

/// 触发插件事件；插件系统未初始化时静默跳过（无插件 = 零开销路径）。
pub(crate) async fn fire_plugin_event<E: qexed_plugin_api::Event>(event: &E) -> qexed_plugin_api::EventEmitResult {
    match qexed_plugin::try_plugin_manager() {
        Some(mgr) => mgr.fire(event).await,
        None => qexed_plugin_api::EventEmitResult::default(),
    }
}

/// 从会话玩家状态构造插件事件用的玩家信息快照。
pub(crate) fn plugin_player_info(player: &PlayPlayer) -> qexed_plugin_api::PlayerInfo {
    qexed_plugin_api::PlayerInfo::new(
        player.profile.uuid.to_string(),
        player.profile.username.clone(),
        player.entity_id,
        player.config.dimension_name.clone(),
    )
}