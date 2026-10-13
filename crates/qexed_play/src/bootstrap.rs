//! 入服引导：Join Game 与初始状态流。
//!
//! 顺序对齐 v4 `play/bootstrap.rs::send_initial_player_state`，只保留 v6 已有
//! 协议包对应的部分；库存/权限/插件等 v4 步骤在对应 crate 落地后补回；命令树已由 qexed_command 落地。

use std::sync::{Arc, Mutex};

use qexed_command::{argument, literal, CommandTree};
use qexed_packet::net_types::{VarInt, VarLong};
use qexed_protocol::to_client::play as s2c;
use qexed_protocol::to_client::play::commands::CommandArgumentType;

use crate::error::PlayError;
use crate::session::PlayPlayer;

/// 发送 Join Game（Login 包）。这是客户端进入 PLAY 阶段后的第一个包。
pub async fn send_join_game<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    player: &PlayPlayer,
    chat_cfg: &qexed_chat::config::ChatConfig,
    chat_session: Option<&qexed_protocol::types::ChatSessionData>,
) -> Result<(), PlayError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let cfg = &player.config;
    sink.send(s2c::login::Login {
        entity_id: player.entity_id,
        is_hardcore: false,
        dimension_names: vec![cfg.dimension_name.clone()],
        max_player: VarInt(20),
        view_distance: VarInt(cfg.view_distance),
        simulation_distance: VarInt(cfg.simulation_distance),
        reduced_debug_info: false,
        enable_respawn_screen: true,
        do_limited_crafting: false,
        dimension_type: VarInt(cfg.dimension_type),
        dimension_name: cfg.dimension_name.clone(),
        hashed_seed: cfg.hashed_seed,
        game_mode: player.game_mode,
        previous_game_mode: -1,
        is_debug: false,
        is_flat: true,
        has_death_location: false,
        death_dimension_name: None,
        death_position: None,
        portal_cooldown: VarInt(0),
        sea_level: VarInt(63),
        // 与 qexed_chat 的安全聊天策略同源：chat.toml 的 secure_chat_enabled。
        // 启用后客户端按签名链严格校验我们转发的 PlayerChat，两处必须一致。
        enforces_secure_chat: chat_cfg.secure_chat_enabled,
    })
    .await?;

    // 玩家列表：把自己加进去（对齐 vanilla：Join Game 后立刻 PlayerInfoUpdate）。
    // 入服此刻会话尚未上报（chat_session 恒为 None）；客户端随后发来的
    // ChatSessionUpdate 会在 play 主循环里触发一次 INITIALIZE_CHAT 补发。
    sink.send(s2c::player_info_update::PlayerInfoUpdate {
        actions: s2c::player_info_update::PlayerInfoActions::player_initializing(),
        entries: vec![{
            let mut entry = s2c::player_info_update::PlayerInfoEntry::from_profile(
                &player.profile,
                player.game_mode as i32,
            );
            entry.chat_session = chat_session.cloned();
            entry
        }],
    })
    .await?;
    Ok(())
}

/// Join Game 之后的初始状态流。顺序敏感：客户端在收到
/// `GameStateChange(level_chunks_load_start)` 前不应看到世界数据。
pub async fn send_initial_state<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    player: &PlayPlayer,
) -> Result<(), PlayError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let cfg = &player.config;

    // 1. 难度（锁定为 peaceable 之外的可调值；单人实现固定 normal）
    sink.send(s2c::change_difficulty::ChangeDifficulty {
        difficulty: 2,
        locked: true,
    })
    .await?;

    // 2. 玩家能力（创造默认可飞行）
    sink.send(s2c::player_abilities::PlayerAbilities {
        flags: s2c::player_abilities::PlayerAbilities::CAN_FLY
            | s2c::player_abilities::PlayerAbilities::INSTABUILD,
        flying_speed: 0.05,
        walking_speed: 0.1,
    })
    .await?;

    // 3. 快捷栏选中格
    sink.send(s2c::set_held_slot::SetHeldSlot { slot: VarInt(0) })
        .await?;

    // 4. 经验条（空）
    sink.send(s2c::set_experience::SetExperience {
        experience_progress: 0.0,
        experience_level: VarInt(0),
        total_experience: VarInt(0),
    })
    .await?;

    // 5. MOTD（客户端 Tab 列表头部区域的数据源之一）
    sink.send(s2c::server_data::ServerData {
        motd: qexed_chat::text_component("qexed server"),
        icon_bytes: None,
    })
    .await?;

    // 6. 世界边界（默认无限大）
    sink.send(s2c::initialize_border::InitializeBorder::default())
        .await?;

    // 7. 时间（26.1+：clock_updates 对应 world_clock 注册表条目）
    //    只发 overworld（id 0）：客户端对未同步的 clock 容忍缺失，
    //    日照时间由 overworld clock 的 total_ticks 驱动（对齐 Pumpkin CUpdateTime::new）
    sink.send(s2c::set_time::SetTime {
        game_time: 0,
        clock_updates: vec![s2c::set_time::ClockUpdate {
            clock: VarInt(0),
            total_ticks: VarLong(0),
            partial_tick: 0.0,
            rate: 1.0,
        }],
    })
    .await?;

    // 8. 默认出生点
    sink.send(s2c::set_default_spawn_position::SetDefaultSpawnPosition {
        dimension: cfg.dimension_name.clone(),
        position: qexed_packet::net_types::Position {
            x: player.position.0 as i32,
            y: player.position.1 as i32,
            z: player.position.2 as i32,
        }, // net_types::Position { x, y, z }
        yaw: 0.0,
        pitch: 0.0,
    })
    .await?;

    // 10. “开始加载区块”信号
    sink.send(s2c::game_state_change::GameStateChange {
        reason: 13, // level_chunks_load_start
        game_mode: 0.0,
    })
    .await?;

    // 11. 刻状态
    sink.send(s2c::ticking_state::TickingState::default())
        .await?;

    // 12. 生命值（创造模式无意义但客户端仍期望收到）
    sink.send(s2c::set_health::SetHealth {
        health: 20.0,
        food: VarInt(20),
        saturation: 5.0,
    })
    .await?;

    // 13. 模拟距离 / 视距 / 视点区块
    sink.send(s2c::set_simulation_distance::SetSimulationDistance {
        simulation_distance: VarInt(cfg.simulation_distance),
    })
    .await?;
    sink.send(s2c::update_view_distance::UpdateViewDistance {
        view_distance: VarInt(cfg.view_distance),
    })
    .await?;
    let (chunk_x, chunk_z) = player.chunk_pos();
    sink.send(s2c::update_view_position::UpdateViewPosition {
        chunk_x: VarInt(chunk_x),
        chunk_z: VarInt(chunk_z),
    })
    .await?;

    // 14. 位置同步（触发客户端 AcceptTeleportation 回应）
    sink.send(s2c::position::Position {
        teleport_id: VarInt(0),
        x: player.position.0,
        y: player.position.1,
        z: player.position.2,
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        yaw: player.rotation.0,
        pitch: player.rotation.1,
        flags: 0,
    })
    .await?;

    Ok(())
}

/// 指令执行产生、需要主循环代发的副作用（executor 在树内运行，
/// 不直接持有 sink，因此先收集后处理）。
#[derive(Debug, Clone, PartialEq)]
pub enum CommandEffect {
    /// 系统聊天回显。
    Reply(String),
    /// 切换游戏模式（GameStateChange reason 3）。
    SetGameMode(u8),
    /// 记分板：需要下发的客户端同步包。
    ScoreboardPacket(qexed_scoreboard::SyncPacket),
    /// 记分板：状态机拒绝了某个操作，转错误回显。
    ScoreboardRejected(String),
}

/// /help 回显的内置指令清单（与 [default_command_tree] 保持一致）。
pub const COMMAND_LIST: &str =
    "/ping, /gamemode <mode>, /say <message>, /scoreboard, /profile <start|stop|report>, /help";

/// /profile 动作执行：start / stop / report（采样剖析器，HTML 报告落盘）。
///
/// 报告写服务器工作目录 `profile_report.html`（对齐 v4 console 语义）。
fn run_profile_action(action: &str) -> String {
    use std::fmt::Write as _;
    let Some(p) = qexed_profiler::global() else {
        return "Profiler not initialized".into();
    };
    match action.trim() {
        "start" => {
            p.reset();
            p.enable();
            "Profiler started (reset). Run /profile report to generate.".into()
        }
        "stop" => {
            p.disable();
            "Profiler stopped.".into()
        }
        "report" => {
            p.disable();
            let html = p.report_html();
            let path = std::env::current_dir()
                .unwrap_or_default()
                .join("profile_report.html");
            match std::fs::write(&path, &html) {
                Ok(()) => {
                    let mut msg = String::from("Report written: ");
                    let _ = write!(msg, "{}", path.display());
                    msg
                }
                Err(e) => {
                    let mut msg = String::from("Report failed: ");
                    let _ = write!(msg, "{e}");
                    msg
                }
            }
        }
        other => {
            let mut msg = String::from("Usage: /profile <start|stop|report> (got: ");
            let _ = write!(msg, "{other})");
            msg
        }
    }
}

/// 内置演示指令树：/ping /gamemode <mode> /say <message> /scoreboard /help。
///
/// executor 通过共享队列上报 [`CommandEffect`]，由主循环
/// `dispatch` 成功后统一 drain 并发包。
///
/// `scoreboard_ops`：/scoreboard executor 只提交 op 到该队列；
/// 主循环 drain 时用 `qexed_scoreboard::command::apply_op` 应用到
/// 共享 [`qexed_scoreboard::Scoreboard`] 并转同步包。
pub fn default_command_tree(
    effects: Arc<Mutex<Vec<CommandEffect>>>,
    scoreboard_ops: qexed_scoreboard::command::OpQueue,
) -> CommandTree {
    CommandTree::new()
        .command({
            let fx = effects.clone();
            literal("ping").executes(move |_| {
                fx.lock().unwrap().push(CommandEffect::Reply("Pong!".into()));
                Ok(())
            })
        })
        .command({
            let fx = effects.clone();
            literal("gamemode").child(
                argument("mode", CommandArgumentType::MinecraftGamemode, false).executes(move |ctx| {
                    let mode = match ctx.args.text("mode").unwrap_or("") {
                        "survival" | "s" => 0,
                        "creative" | "c" => 1,
                        "adventure" | "a" => 2,
                        "spectator" | "sp" => 3,
                        other => {
                            fx.lock().unwrap()
                                .push(CommandEffect::Reply(format!("Unknown game mode: {other}")));
                            return Ok(());
                        }
                    };
                    fx.lock().unwrap().push(CommandEffect::SetGameMode(mode));
                    Ok(())
                }),
            )
        })
        .command({
            let fx = effects.clone();
            literal("say").child(
                // greedy：吞掉行尾全部剩余文本
                argument("message", CommandArgumentType::BrigadierString(Default::default()), true)
                    .executes(move |ctx| {
                        let msg = ctx.args.text("message").unwrap_or("").to_string();
                        fx.lock().unwrap().push(CommandEffect::Reply(format!("[Server] {msg}")));
                        Ok(())
                    }),
            )
        })
        // /scoreboard objectives|players 全子命令族；动作经 ops 队列上报
        .command(qexed_scoreboard::command::scoreboard_node(scoreboard_ops))
        // /profile start|stop|report：采样剖析器（HTML 报告落盘工作目录）
        .command({
            let fx = effects.clone();
            literal("profile").child(
                argument("action", CommandArgumentType::BrigadierString(Default::default()), true)
                    .executes(move |ctx| {
                        let reply = run_profile_action(ctx.args.text("action").unwrap_or(""));
                        fx.lock().unwrap().push(CommandEffect::Reply(reply));
                        Ok(())
                    }),
            )
        })
        .command({
            let fx = effects.clone();
            literal("help").executes(move |_| {
                fx.lock().unwrap().push(CommandEffect::Reply(format!("Commands: {COMMAND_LIST}")));
                Ok(())
            })
        })
}

/// 生成指令树对应的 Declare Commands 包（同步纯函数）。
///
/// 特意不走 async fn：树内 executor 是 \`dyn FnMut + Send\`（非 Sync），
/// async fn 会把 \`&CommandTree\` 留在 future 里跨 await，导致 future 非 Send，
/// 上层的 \`tokio::spawn\` 直接编译失败。
pub fn command_tree_packet(tree: &CommandTree) -> s2c::commands::Commands {
    tree.to_packet()
}