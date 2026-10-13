//! digfight（掘一死战）：v4 WASM 示例插件 → v6 原生 cdylib 插件移植。
//!
//! v4 导出 → v6 事件映射：
//! - init                  → declare_simple_plugin! init 闭包（建大厅 + 注册命令）
//! - config_reload         → ConfigReloadEvent（宿主未接线，休眠）
//! - player_join           → PlayerJoinEvent（清 session）
//! - player_leave          → PlayerLeaveEvent（弃权判负 + 清 session）
//! - commands              → ctx.register_command("digfight")
//! - command_execute       → ChatCommandEvent（cancel 接管）
//! - npc_mutations         → NpcMutationsEvent（Mutex 回写；宿主未接线，休眠）
//! - npc_interact          → NpcInteractEvent（OpenMenu）
//! - player_tick           → PlayerTickEvent（休眠；阶段转换/缩边/虚空判死）
//! - player_attack         → PlayerAttackEvent（休眠；v4 本身即空壳）
//! - block_drops           → BlockDropsEvent（休眠；战利品替换）
//!
//! 语义差异（相对 v4）：
//! 1. v4 存储键 NPC_KEY "digfight:guide" 含冒号（v6 存储键字符集不允许），改为
//!    digfight_guide；NPC 事件名保持 digfight。
//! 2. v4 config_reload 判定 ends_with("config.toml") 会匹配任意插件的配置；v6 收紧为
//!    digfight/config.toml 或路径含 digfight 的 config.toml（宿主对所有插件广播 reload）。
//! 3. v4 对局结束回大厅/淘汰动作经 response 派发给触发玩家；v6 事件动作只能派发给
//!    手头的 PlayerInfo，跨玩家动作（结算广播、回大厅）改经在线玩家表派发。
//! 4. v4 broadcast_bossbar 返回空 Vec（未实现）；v6 保留同语义空实现。
//! 5. v4 BlockDropItem { item_id: 0, item_name } 即按名字发物品；v6 同款（宿主无物品
//!    名注册通路时 item_id 0 + item_name 语义一致）。
//! 6. v4 priority() = 270 丢弃：v6 事件按注册顺序（FIFO）分发。
//! 7. v4 PlayerSession 存 WASM 线性存储持久化；v6 经 storage JSON（键含 uuid 去横线）。
//! 8. v4 持 runtime 锁时还调 save_game（WASM 下 Mutex 宽容）；v6 原生 std::sync::Mutex
//!    不可重入，持锁分支改为就地修改（get_mut 即持久），进 check_game_end 前先释放锁。
//! 9. v4 开局传送/结算回大厅经 response 只派发给触发玩家；v6 经在线玩家表派发给全部
//!    在线参与者（在线表由 join/leave 事件维护，宿主未接线时为空 → 动作不下发）。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use qexed_plugin_api::payload::{
    BlockDropItem, NpcMutationOp, NpcUpsert, PlayerAction as Pa, PlayerPayloadOwned,
};
use qexed_plugin_api::services::WorldEditRegionSpec;
use qexed_plugin_api::{
    BlockDropsEvent, ChatCommandEvent, ConfigReloadEvent, EventContext, LogLevel, NpcInteractEvent,
    NpcMutationsEvent, PlayerAttackEvent, PlayerInfo, PlayerJoinEvent, PlayerLeaveEvent,
    PlayerTickEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::declare_simple_plugin!("digfight", "Digfight", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        let config = load_config(&sdk);
        register_lobby(&sdk, &config);
        if config.enable {
            build_lobby(&sdk, &config);
        }
        ctx.register_command(COMMAND_NAME);
        ctx.log(LogLevel::Info, "digfight initialized: 掘一死战");

        // —— /digfight 命令 ——
        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != COMMAND_NAME {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    dispatch(&sdk, &ev.player, vec![message("掘一死战已关闭")]);
                    return;
                }
                let arg = ev
                    .argument
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let actions = match arg.as_str() {
                    "" | "menu" => vec![open_menu(&config)],
                    "join" | "queue" => {
                        join_queue(&sdk, &config, &ev.player)
                    }
                    "leave" | "quit" => leave_queue(&sdk, &config, &ev.player.uuid),
                    "stats" | "record" => show_stats(&sdk, &ev.player.uuid),
                    _ => vec![message(
                        "用法: /digfight join | /digfight leave | /digfight stats",
                    )],
                };
                dispatch(&sdk, &ev.player, actions);
            })
        });

        // —— 配置热重载（宿主接线后自动生效）——
        ctx.register::<ConfigReloadEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("digfight/config.toml")
                    || path.ends_with("config.toml") && path.contains("digfight"))
                {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_lobby(&sdk, &config);
                if config.enable {
                    build_lobby(&sdk, &config);
                }
                api.log(LogLevel::Info, "digfight config reloaded");
            })
        });

        // —— 进服清 session ——
        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                register_online(&ev.player);
                let _ = sdk.storage_delete(&session_key(&ev.player.uuid));
            })
        });

        // —— 退服：弃权判负 + 清 session ——
        ctx.register::<PlayerLeaveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                unregister_online(&ev.player.uuid);
                remove_player_from_game(&sdk, &ev.player.uuid);
                let _ = sdk.storage_delete(&session_key(&ev.player.uuid));
            })
        });

        // —— NPC upsert（宿主接线后自动生效）——
        ctx.register::<NpcMutationsEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                ev.response.lock().unwrap().operations = vec![NpcMutationOp::Upsert {
                    npc: guide_npc(&config),
                }];
            })
        });

        // —— NPC 交互 → OpenMenu ——
        ctx.register::<NpcInteractEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let p = &ev.payload;
                if p.entity.key != NPC_KEY || p.configured_event != NPC_EVENT {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let player = player_info(&p.player);
                sdk.perform(&player, open_menu(&config));
            })
        });

        // —— 每 tick：阶段转换 / 缩边 / 虚空判死（宿主接线后自动生效）——
        ctx.register::<PlayerTickEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                let Some(session) = active_session(&sdk, &ev.player.uuid) else {
                    return;
                };
                let Some(game) = current_game(&session.game_id) else {
                    return;
                };
                if !session.alive || game.phase.is_finished() {
                    return;
                }
                let now = now_millis();
                let mut game = game;

                // 挖掘阶段 → 战斗阶段
                if game.phase == GamePhase::DigPhase && now >= game.phase_end_ms {
                    game.phase = GamePhase::FightPhase;
                    game.phase_end_ms = now.saturating_add(config.arena.shrink_start_delay_ms);
                    game.next_shrink_ms = game.phase_end_ms;
                    save_game(&game);
                    let alive = alive_player_list(&game);
                    let actions = broadcast_bossbar(&alive, &game);
                    dispatch(&sdk, &ev.player, actions);
                    return;
                }

                if game.phase == GamePhase::FightPhase {
                    if now < game.next_shrink_ms
                        || game.current_radius <= config.arena.min_radius.max(2)
                    {
                        return;
                    }
                    game.next_shrink_ms = now.saturating_add(config.arena.shrink_interval_ms);
                    shrink_border(&sdk, &config, &mut game);

                    // 缩边后检查触发者是否掉出竞技场
                    let mut killed_players = Vec::new();
                    let min_y = f64::from(game.origin_y) - 10.0;
                    if game.alive_map.get(&ev.player.uuid) == Some(&true)
                        && ev.position.y < min_y
                    {
                        killed_players.push(ev.player.uuid.clone());
                    }
                    for player_id in &killed_players {
                        game.alive_map.insert(player_id.clone(), false);
                        game.death_messages
                            .push(format!("{} 掉出了竞技场", player_name(&game, player_id)));
                    }
                    save_game(&game);
                    let alive = alive_player_list(&game);
                    if let Some(end_actions) = check_game_end(&sdk, &config, &mut game, &alive) {
                        dispatch(&sdk, &ev.player, end_actions);
                        return;
                    }
                    let radius = game.current_radius;
                    let mut actions = broadcast_bossbar(&alive, &game);
                    actions.push(message(format!(
                        "边界塌陷！剩余半径 {} 格，存活 {} 人",
                        radius,
                        alive.len()
                    )));
                    dispatch(&sdk, &ev.player, actions);
                }
            })
        });

        // —— 攻击（宿主接线后自动生效）——
        // v4 同款空壳：伤害放行交给宿主，击杀判定走 tick 虚空检测。
        ctx.register::<PlayerAttackEvent>(move |_api, _ev, _| {
            Box::pin(async move {})
        });

        // —— 方块掉落替换（宿主接线后自动生效）——
        ctx.register::<BlockDropsEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                let Some(player_ref) = ev.query.player.clone() else {
                    return;
                };
                let Some(session) = active_session(&sdk, &player_ref.uuid) else {
                    return;
                };
                let Some(game) = current_game(&session.game_id) else {
                    return;
                };
                if game.phase.is_finished() {
                    return;
                }
                let Some(loot) = pick_loot(&config, &ev.query.block_name) else {
                    return;
                };
                // v6 现行契约：handler 只拿 &E，BlockDropsEvent.response 非 Mutex，
                // 暂无法回写 replace/items；宿主提供可写 response 后替换即自动生效。
                api.log(
                    LogLevel::Info,
                    format!("digfight: {} 挖出 {}（战利品回写待宿主接线）", player_ref.username, loot)
                        .as_str(),
                );
                let _ = BlockDropItem {
                    item_id: 0, // 按名字发放（v4 BlockDropItem item_id 0 + item_name 语义）
                    item_name: loot,
                    count: 1,
                };
            })
        });
    })
});

const COMMAND_NAME: &str = "digfight";
const NPC_KEY: &str = "digfight_guide"; // v4 "digfight:guide"：v6 存储键不允许冒号
const NPC_EVENT: &str = "digfight";
const SESSION_PREFIX: &str = "digfight/session/";
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;

// ============ NPC ============

fn guide_npc(config: &Config) -> NpcUpsert {
    NpcUpsert {
        key: NPC_KEY.to_string(),
        dimension: config.lobby.dimension.clone(),
        x: config.lobby.npc_x,
        y: config.lobby.npc_y,
        z: config.lobby.npc_z,
        yaw: config.lobby.npc_yaw,
        pitch: config.lobby.npc_pitch,
        name: "掘一死战".to_string(),
        display_name: config.lobby.npc_display_name.clone(),
        entity_type: config.lobby.npc_entity_type.clone(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        look_at_players: true,
        main_hand_event: NPC_EVENT.to_string(),
        off_hand_event: NPC_EVENT.to_string(),
        attack_event: NPC_EVENT.to_string(),
    }
}

// ============ 队列 ============

fn join_queue(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player: &PlayerInfo) -> Vec<Pa> {
    let player_uuid = player.uuid.as_str();
    let username = player.username.as_str();
    if let Some(session) = active_session(sdk, player_uuid) {
        if session.alive {
            if let Some(game) = current_game(&session.game_id) {
                if !game.phase.is_finished() {
                    return vec![message("你已经在游戏中！")];
                }
            }
        }
    }

    let game_id = {
        let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
        // 找到或创建排队中的游戏
        let queue_game = runtime
            .games
            .values_mut()
            .find(|game| game.phase == GamePhase::Queuing);
        match queue_game {
            Some(game) => {
                if game.player_ids.iter().any(|id| id == player_uuid) {
                    return vec![message("你已经在排队中！")];
                }
                game.player_ids.push(player_uuid.to_string());
                game.player_names
                    .insert(player_uuid.to_string(), username.to_string());
                game.alive_map.insert(player_uuid.to_string(), true);
                game.participants.insert(player_uuid.to_string(), true);
                game.id.clone()
            }
            None => {
                let id = format!("game_{}", now_millis());
                let game = GameState {
                    id: id.clone(),
                    phase: GamePhase::Queuing,
                    phase_end_ms: 0,
                    next_shrink_ms: 0,
                    current_radius: 0,
                    player_ids: vec![player_uuid.to_string()],
                    player_names: HashMap::from([(
                        player_uuid.to_string(),
                        username.to_string(),
                    )]),
                    alive_map: HashMap::from([(player_uuid.to_string(), true)]),
                    participants: HashMap::from([(player_uuid.to_string(), true)]),
                    origin_x: 0,
                    origin_y: 0,
                    origin_z: 0,
                    death_messages: Vec::new(),
                };
                runtime.games.insert(id.clone(), game);
                id
            }
        }
    };

    let queued = current_game(&game_id)
        .map(|game| game.player_ids.len())
        .unwrap_or(0);
    let needed = config.arena.min_players.max(2) as usize;

    let _ = sdk.storage_set_json(
        &session_key(player_uuid),
        &PlayerSession {
            game_id: game_id.clone(),
            alive: true,
            kills: 0,
        },
    );

    let mut actions = vec![message(format!(
        "你已加入掘一死战队列 ({}/{})，等待更多玩家...",
        queued, needed
    ))];

    if queued >= needed {
        // 满员开局
        actions.extend(start_game(sdk, config, &game_id, player));
    }

    actions
}

fn leave_queue(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> Vec<Pa> {
    let Some(session) = active_session(sdk, player_uuid) else {
        return vec![message("你不在队列或游戏中")];
    };

    let game_id = session.game_id.clone();
    let was_alive = session.alive;
    let _ = sdk.storage_delete(&session_key(player_uuid));

    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    if let Some(game) = runtime.games.get_mut(&game_id) {
        if game.phase == GamePhase::Queuing {
            game.player_ids.retain(|id| id != player_uuid);
            game.player_names.remove(player_uuid);
            game.alive_map.remove(player_uuid);
            // 就地修改即持久，无需再调 save_game（避免重入死锁）
            drop(runtime);
            return vec![message("你已离开队列")];
        }
        if was_alive && !game.phase.is_finished() {
            game.alive_map.insert(player_uuid.to_string(), false);
            game.death_messages.push(format!(
                "{} 退出了游戏",
                game.player_names
                    .get(player_uuid)
                    .cloned()
                    .unwrap_or_else(|| player_uuid.to_string())
            ));
            // 进 check_game_end 前先释放锁
            drop(runtime);
            if let Some(game) = current_game(&game_id) {
                let alive = alive_player_list(&game);
                let mut game = game;
                check_game_end(sdk, config, &mut game, &alive);
            }
            return vec![
                message("已退出游戏，正在返回大厅"),
                Pa::SetPlayersVisible { visible: true },
                Pa::Teleport {
                    dimension: config.lobby.dimension.clone(),
                    x: config.lobby.spawn_x,
                    y: config.lobby.spawn_y,
                    z: config.lobby.spawn_z,
                    yaw: Some(config.lobby.spawn_yaw),
                    pitch: Some(config.lobby.spawn_pitch),
                },
            ];
        }
    }

    vec![
        message("已退出游戏，正在返回大厅"),
        Pa::SetPlayersVisible { visible: true },
        Pa::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
    ]
}


// ============ 游戏生命周期 ============

fn start_game(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    game_id: &str,
    trigger: &PlayerInfo,
) -> Vec<Pa> {
    let (origin, player_ids, count) = {
        let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
        let Some(game) = runtime.games.get_mut(game_id) else {
            return Vec::new();
        };

        let origin = (
            config.arena.origin_x,
            config.arena.origin_y,
            config.arena.origin_z,
        );
        game.origin_x = origin.0;
        game.origin_y = origin.1;
        game.origin_z = origin.2;
        game.current_radius = config.arena.initial_radius.max(config.arena.min_radius + 2);

        // 登记编辑区 + 建造竞技场
        register_arena_region(sdk, config, game);
        if !build_arena(sdk, config, game) {
            game.phase = GamePhase::Queuing; // 建造失败：退回排队，允许重新凑人开局
            return vec![message("竞技场建造失败，请稍后重试")];
        }

        let now = now_millis();
        game.phase = GamePhase::DigPhase;
        game.phase_end_ms = now.saturating_add(config.arena.dig_phase_ms);

        (origin, game.player_ids.clone(), game.player_ids.len())
    };

    let broadcast_text = message(format!(
        "掘一死战开始！{} 名玩家进入竞技场，挖掘阶段 {} 秒！",
        count,
        config.arena.dig_phase_ms / 1000
    ));

    let mut actions = Vec::new();

    // 逐个参与者：会话 + 各自散点传送。v4 全部动作（N 个传送叠加）经 response
    // 只发给触发玩家；v6 按人派发各自的广播 + 传送（在线表，join/leave 维护）。
    for (i, player_id) in player_ids.iter().enumerate() {
        let angle = (i as f64) * (std::f64::consts::TAU / count as f64);
        let spawn_r = 3.0;
        let spawn_x = origin.0 as f64 + 0.5 + angle.cos() * spawn_r;
        let spawn_z = origin.2 as f64 + 0.5 + angle.sin() * spawn_r;

        let _ = sdk.storage_set_json(
            &session_key(player_id),
            &PlayerSession {
                game_id: game_id.to_string(),
                alive: true,
                kills: 0,
            },
        );

        let per_player = vec![
            broadcast_text.clone(),
            Pa::SetPlayersVisible { visible: true },
            Pa::Teleport {
                dimension: config.arena.dimension.clone(),
                x: spawn_x,
                y: (origin.1 + config.arena.layer_count.max(0) + 1) as f64,
                z: spawn_z,
                yaw: Some(0.0),
                pitch: Some(0.0),
            },
        ];
        match online_player(player_id) {
            Some(target) => dispatch(sdk, &target, per_player),
            None => {
                // 不在在线表（宿主未接线 join）：触发玩家经命令响应兜底
                if player_id == trigger.uuid.as_str() {
                    actions.extend(per_player);
                }
            }
        }
    }

    actions
}


fn check_game_end(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    game: &mut GameState,
    alive: &[String],
) -> Option<Vec<Pa>> {
    if alive.len() > 1 {
        return None;
    }

    let winner = alive.first().cloned();
    game.phase = GamePhase::Finished;
    save_game(game);

    let winner_name = winner
        .as_ref()
        .and_then(|id| game.player_names.get(id).cloned())
        .unwrap_or_else(|| "无".to_string());

    let death_log = if game.death_messages.is_empty() {
        String::new()
    } else {
        format!("\n淘汰记录:\n{}", game.death_messages.join("\n"))
    };

    // 收集结算信息后立刻释放 runtime 锁，再做存储/派发等耗时操作
    let participant_ids: Vec<String> = game.participants.keys().cloned().collect();
    let summary = message(format!(
        "掘一死战结束！获胜者: {}{}",
        winner_name, death_log
    ));

    let mut actions = vec![Pa::SetPlayersVisible { visible: true }, summary];

    // 逐个参与者：清 session + 回大厅传送（按在线表派发）
    for player_id in &participant_ids {
        let _ = sdk.storage_delete(&session_key(player_id));
        if let Some(target) = online_player(player_id) {
            dispatch(
                sdk,
                &target,
                vec![Pa::Teleport {
                    dimension: config.lobby.dimension.clone(),
                    x: config.lobby.spawn_x,
                    y: config.lobby.spawn_y,
                    z: config.lobby.spawn_z,
                    yaw: Some(config.lobby.spawn_yaw),
                    pitch: Some(config.lobby.spawn_pitch),
                }],
            );
        }
    }
    actions.push(message("全部参与者已传送回大厅"));

    // 清理 runner 实体引用（v4 遗留键，v6 保留同语义清理）
    for player_id in &participant_ids {
        let _ = sdk.entity_remove(&runner_key(player_id));
    }

    Some(actions)
}

fn remove_player_from_game(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) {
    let Some(session) = active_session(sdk, player_uuid) else {
        return;
    };
    let _ = sdk.entity_remove(&runner_key(player_uuid));

    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    if let Some(game) = runtime.games.get_mut(&session.game_id) {
        if session.alive && !game.phase.is_finished() {
            game.alive_map.insert(player_uuid.to_string(), false);
            game.death_messages.push(format!(
                "{} 离开了游戏",
                game.player_names
                    .get(player_uuid)
                    .cloned()
                    .unwrap_or_else(|| player_uuid.to_string())
            ));
            // 就地修改即持久（避免 get_mut 后再 save_game 重入死锁）
        }
    }
}

// ============ 竞技场建造 ============

fn register_arena_region(sdk: &qexed_plugin_sdk::Sdk, config: &Config, game: &GameState) {
    let radius = game.current_radius + 4;
    let height = config.arena.layer_count.max(1) + 4;
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id: format!("digfight_{}", game.id),
        dimension: config.arena.dimension.clone(),
        min: (
            game.origin_x - radius,
            game.origin_y - 2,
            game.origin_z - radius,
        ),
        max: (
            game.origin_x + radius,
            game.origin_y + height,
            game.origin_z + radius,
        ),
        allow_player_break: true,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_arena(sdk: &qexed_plugin_sdk::Sdk, config: &Config, game: &GameState) -> bool {
    let radius = game.current_radius;
    let layers = config.arena.layer_count.max(1);
    let dim = config.arena.dimension.as_str();
    let mut blocks: Vec<(&str, (i32, i32, i32), &str)> = Vec::new();
    let origin = (game.origin_x, game.origin_y, game.origin_z);

    // 分层竞技场地板 + 边界墙
    for dy in 0..=layers {
        let y = origin.1 + dy;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() == radius || dz.abs() == radius {
                    // 边界：不可破坏的 barrier 墙
                    blocks.push((dim, (origin.0 + dx, y, origin.2 + dz), "minecraft:barrier"));
                } else if dy < layers {
                    // 可挖掘层
                    let block = match dy {
                        0 => config.arena.deep_layer_block.as_str(),
                        1 => config.arena.mid_layer_block.as_str(),
                        _ => config.arena.top_layer_block.as_str(),
                    };
                    blocks.push((dim, (origin.0 + dx, y, origin.2 + dz), block));
                }
            }
        }
    }

    // 清空竞技场上方
    for dy in (layers + 1)..=(layers + 6) {
        let y = origin.1 + dy;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                blocks.push((dim, (origin.0 + dx, y, origin.2 + dz), "minecraft:air"));
            }
        }
    }

    // 按 stable_seed 散布传说战利品特殊方块
    let special_count = (radius * radius / 6).max(5) as usize;
    let mut hash = stable_seed(&game.id, 0);
    for _ in 0..special_count {
        hash = hash
            .wrapping_mul(0x100000001b3)
            .wrapping_add(0xcbf29ce484222325);
        let x = ((hash % (radius as u64 * 2)) as i32) - radius;
        hash = hash
            .wrapping_mul(0x100000001b3)
            .wrapping_add(0xcbf29ce484222325);
        let z = ((hash % (radius as u64 * 2)) as i32) - radius;
        let layer = (hash % layers as u64) as i32;
        let y = origin.1 + layer;
        blocks.push((
            dim,
            (origin.0 + x, y, origin.2 + z),
            config.arena.special_block.as_str(),
        ));
    }

    // 分批写入（world_set_blocks 返回成功写入数）
    for chunk in blocks.chunks(WORLD_EDIT_BATCH_LIMIT) {
        if sdk.world_set_blocks(chunk) < chunk.len() {
            return false;
        }
    }
    true
}

fn shrink_border(sdk: &qexed_plugin_sdk::Sdk, config: &Config, game: &mut GameState) {
    let new_radius =
        (game.current_radius - config.arena.shrink_step).max(config.arena.min_radius.max(2));
    if new_radius >= game.current_radius {
        return;
    }
    game.current_radius = new_radius;

    let dim = config.arena.dimension.as_str();
    let origin = (game.origin_x, game.origin_y, game.origin_z);
    let layers = config.arena.layer_count.max(1);

    let mut blocks: Vec<(&str, (i32, i32, i32), &str)> = Vec::new();
    let old_radius = new_radius + config.arena.shrink_step;
    for dy in 0..=layers + 2 {
        let y = origin.1 + dy;
        for r in new_radius..old_radius {
            for sign in [-1, 1] {
                // 上下两条边
                for dx in -r..=r {
                    blocks.push((
                        dim,
                        (origin.0 + dx, y, origin.2 + sign * r),
                        "minecraft:air",
                    ));
                }
                // 左右两条边（角已由上一步覆盖）
                for dz in -(r - 1)..=(r - 1) {
                    blocks.push((
                        dim,
                        (origin.0 + sign * r, y, origin.2 + dz),
                        "minecraft:air",
                    ));
                }
            }
        }
        // 新边界放 barrier 墙
        if dy <= layers {
            for dx in -new_radius..=new_radius {
                blocks.push((
                    dim,
                    (origin.0 + dx, y, origin.2 + new_radius),
                    "minecraft:barrier",
                ));
                blocks.push((
                    dim,
                    (origin.0 + dx, y, origin.2 - new_radius),
                    "minecraft:barrier",
                ));
            }
            for dz in -(new_radius - 1)..=(new_radius - 1) {
                blocks.push((
                    dim,
                    (origin.0 + new_radius, y, origin.2 + dz),
                    "minecraft:barrier",
                ));
                blocks.push((
                    dim,
                    (origin.0 - new_radius, y, origin.2 + dz),
                    "minecraft:barrier",
                ));
            }
        }
    }

    for chunk in blocks.chunks(WORLD_EDIT_BATCH_LIMIT) {
        let _ = sdk.world_set_blocks(chunk);
    }
}

// ============ 战利品 ============

fn pick_loot(config: &Config, block_name: &str) -> Option<String> {
    if block_name == config.arena.special_block {
        Some(lottery_roll_single(&config.loot.special_items)?)
    } else if block_name == config.arena.top_layer_block {
        Some(lottery_roll_single(&config.loot.common_items)?)
    } else if block_name == config.arena.mid_layer_block {
        Some(lottery_roll_single(&config.loot.uncommon_items)?)
    } else if block_name == config.arena.deep_layer_block {
        Some(lottery_roll_single(&config.loot.rare_items)?)
    } else {
        None
    }
}

fn lottery_roll_single(items: &[String]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let mut entries: Vec<(String, u64)> = Vec::new();
    for item in items {
        let mut parts = item.split(':');
        let Some(item_name) = parts.next() else {
            continue;
        };
        let Some(weight) = parts.next().and_then(|w| w.parse::<u64>().ok()) else {
            continue;
        };
        entries.push((item_name.to_string(), weight));
    }
    if entries.is_empty() {
        return None;
    }

    let total: u64 = entries.iter().map(|(_, w)| w).sum();
    if total == 0 {
        return entries.first().map(|(name, _)| name.clone());
    }
    let seed = now_millis() as u64;
    let mut roll = seed.wrapping_mul(0x9e3779b97f4a7c15) % total;
    for (name, weight) in &entries {
        if roll < *weight {
            return Some(name.clone());
        }
        roll -= *weight;
    }
    entries.last().map(|(name, _)| name.clone())
}

// ============ 大厅 ============

fn register_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let y = config.lobby.floor_y;
    let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
        id: "digfight_lobby".to_string(),
        dimension: config.lobby.dimension.clone(),
        min: (-12, y - 1, -12),
        max: (12, y + 6, 12),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let y = config.lobby.floor_y;
    let dim = config.lobby.dimension.as_str();
    let mut blocks: Vec<(&str, (i32, i32, i32), &str)> = Vec::new();
    for x in -5..=5 {
        for z in -5..=5 {
            blocks.push((dim, (x, y - 1, z), "minecraft:polished_andesite"));
            for air_y in y..=y + 4 {
                blocks.push((dim, (x, air_y, z), "minecraft:air"));
            }
        }
    }
    blocks.push((dim, (0, y, 0), "minecraft:sea_lantern"));
    let _ = sdk.world_set_blocks(&blocks);
}
// ============ 工具 ============

fn current_game(game_id: &str) -> Option<GameState> {
    let runtime = runtime_state().lock().expect("digfight runtime poisoned");
    runtime.games.get(game_id).cloned()
}

fn save_game(game: &GameState) {
    let mut runtime = runtime_state().lock().expect("digfight runtime poisoned");
    runtime.games.insert(game.id.clone(), game.clone());
}

fn active_session(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) -> Option<PlayerSession> {
    sdk.storage_get_json::<PlayerSession>(&session_key(player_uuid))
}

fn alive_player_list(game: &GameState) -> Vec<String> {
    game.player_ids
        .iter()
        .filter(|id| game.alive_map.get(*id) == Some(&true))
        .cloned()
        .collect()
}

fn broadcast_bossbar(alive: &[String], game: &GameState) -> Vec<Pa> {
    let _phase_text = match game.phase {
        GamePhase::DigPhase => "挖掘阶段",
        GamePhase::FightPhase => "战斗阶段",
        _ => "",
    };
    let _title = format!(
        "掘一死战 | {} | {} | 存活 {} 人 | 半径 {}",
        _phase_text,
        format_countdown(game.phase_end_ms),
        alive.len(),
        game.current_radius
    );
    Vec::new() // v4 同款空实现：BossBar 为逐玩家动作，v4 未实现，v6 保留同语义
}

fn format_countdown(end_ms: i64) -> String {
    let remaining = (end_ms - now_millis()).max(0) / 1000;
    format!("{}:{:02}", remaining / 60, remaining % 60)
}

fn player_name(game: &GameState, player_id: &str) -> String {
    game.player_names
        .get(player_id)
        .cloned()
        .unwrap_or_else(|| player_id.to_string())
}

fn show_stats(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) -> Vec<Pa> {
    match active_session(sdk, player_uuid) {
        Some(s) => vec![message(format!(
            "你在游戏 [{}] 中，状态: {}，击杀: {}",
            s.game_id,
            if s.alive { "存活" } else { "已淘汰" },
            s.kills
        ))],
        None => vec![message("你当前不在游戏中")],
    }
}

// ============ 键与杂项 ============

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{}", player_uuid.replace('-', ""))
}

fn runner_key(player_uuid: &str) -> String {
    format!("digfight/runner/{}", player_uuid.replace('-', ""))
}

fn stable_seed(value: &str, salt: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64 ^ salt;
    for byte in value.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn open_menu(config: &Config) -> Pa {
    Pa::OpenMenu {
        menu: config.menu_id.clone(),
    }
}

fn message(text: impl Into<String>) -> Pa {
    Pa::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn player_info(p: &PlayerPayloadOwned) -> PlayerInfo {
    PlayerInfo::new(
        p.uuid.clone(),
        p.username.clone(),
        p.entity_id,
        p.dimension.clone(),
    )
}

/// 在线玩家快照（join/leave 维护）。v4 response 可"稍后按 uuid 下发"给任意玩家；
/// v6 动作派发需要 &PlayerInfo，开局传送/结算回大厅要发给非触发玩家，故维护这张表。
fn online_players() -> &'static Mutex<Vec<PlayerInfo>> {
    static PLAYERS: OnceLock<Mutex<Vec<PlayerInfo>>> = OnceLock::new();
    PLAYERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn register_online(player: &PlayerInfo) {
    let mut players = online_players().lock().unwrap();
    players.retain(|p| p.uuid != player.uuid);
    players.push(player.clone());
}

fn unregister_online(player_uuid: &str) {
    online_players().lock().unwrap().retain(|p| p.uuid != player_uuid);
}

fn online_player(uuid: &str) -> Option<PlayerInfo> {
    online_players()
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.uuid == uuid)
        .cloned()
}

fn dispatch(sdk: &qexed_plugin_sdk::Sdk, player: &PlayerInfo, actions: Vec<Pa>) {
    if actions.is_empty() {
        return;
    }
    sdk.perform_all(&actions.iter().map(|a| (player, a.clone())).collect::<Vec<_>>());
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn load_config(sdk: &qexed_plugin_sdk::Sdk) -> Config {
    sdk.read_config()
        .ok()
        .flatten()
        .and_then(|contents| contents.parse::<toml_edit::DocumentMut>().ok())
        .and_then(|document| qexed_toml::from_document::<Config>(document).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    DEFAULT_CONFIG
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|document| qexed_toml::from_document::<Config>(document).ok())
        .expect("default digfight config is valid")
}

// ============ 运行时状态 ============

#[derive(Default)]
struct RuntimeState {
    games: HashMap<String, GameState>,
}

fn runtime_state() -> &'static Mutex<RuntimeState> {
    static STATE: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RuntimeState::default()))
}

// ============ 数据类型 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSession {
    game_id: String,
    alive: bool,
    kills: i32,
}

#[derive(Debug, Clone)]
struct GameState {
    id: String,
    phase: GamePhase,
    phase_end_ms: i64,
    next_shrink_ms: i64,
    current_radius: i32,
    player_ids: Vec<String>,
    player_names: HashMap<String, String>,
    alive_map: HashMap<String, bool>,
    participants: HashMap<String, bool>,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    death_messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GamePhase {
    Queuing,
    DigPhase,
    FightPhase,
    Finished,
}

impl GamePhase {
    fn is_finished(self) -> bool {
        matches!(self, GamePhase::Finished)
    }
}
// ============ 配置 ============

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    arena: ArenaConfig,
    #[serde(default)]
    loot: LootConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: true,
            menu_id: default_menu_id(),
            lobby: LobbyConfig::default(),
            arena: ArenaConfig::default(),
            loot: LootConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default)]
    spawn_x: f64,
    #[serde(default = "default_lobby_y")]
    spawn_y: f64,
    #[serde(default)]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "default_lobby_floor_y")]
    floor_y: i32,
    #[serde(default)]
    npc_x: f64,
    #[serde(default = "default_lobby_y")]
    npc_y: f64,
    #[serde(default = "default_npc_z")]
    npc_z: f64,
    #[serde(default = "default_npc_yaw")]
    npc_yaw: f32,
    #[serde(default)]
    npc_pitch: f32,
    #[serde(default = "default_npc_entity_type")]
    npc_entity_type: String,
    #[serde(default = "default_npc_display_name")]
    npc_display_name: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            dimension: default_lobby_dimension(),
            spawn_x: 0.5,
            spawn_y: default_lobby_y(),
            spawn_z: 0.5,
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_lobby_floor_y(),
            npc_x: 0.0,
            npc_y: default_lobby_y(),
            npc_z: default_npc_z(),
            npc_yaw: default_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: default_npc_entity_type(),
            npc_display_name: default_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ArenaConfig {
    #[serde(default = "default_arena_dimension")]
    dimension: String,
    #[serde(default = "default_arena_origin_x")]
    origin_x: i32,
    #[serde(default = "default_arena_origin_y")]
    origin_y: i32,
    #[serde(default = "default_arena_origin_z")]
    origin_z: i32,
    #[serde(default = "default_min_players")]
    min_players: i32,
    #[serde(default = "default_initial_radius")]
    initial_radius: i32,
    #[serde(default = "default_min_radius")]
    min_radius: i32,
    #[serde(default = "default_shrink_step")]
    shrink_step: i32,
    #[serde(default = "default_shrink_interval_ms")]
    shrink_interval_ms: i64,
    #[serde(default = "default_shrink_start_delay_ms")]
    shrink_start_delay_ms: i64,
    #[serde(default = "default_dig_phase_ms")]
    dig_phase_ms: i64,
    #[serde(default = "default_layer_count")]
    layer_count: i32,
    #[serde(default = "default_top_layer_block")]
    top_layer_block: String,
    #[serde(default = "default_mid_layer_block")]
    mid_layer_block: String,
    #[serde(default = "default_deep_layer_block")]
    deep_layer_block: String,
    #[serde(default = "default_special_block")]
    special_block: String,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            dimension: default_arena_dimension(),
            origin_x: default_arena_origin_x(),
            origin_y: default_arena_origin_y(),
            origin_z: default_arena_origin_z(),
            min_players: default_min_players(),
            initial_radius: default_initial_radius(),
            min_radius: default_min_radius(),
            shrink_step: default_shrink_step(),
            shrink_interval_ms: default_shrink_interval_ms(),
            shrink_start_delay_ms: default_shrink_start_delay_ms(),
            dig_phase_ms: default_dig_phase_ms(),
            layer_count: default_layer_count(),
            top_layer_block: default_top_layer_block(),
            mid_layer_block: default_mid_layer_block(),
            deep_layer_block: default_deep_layer_block(),
            special_block: default_special_block(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LootConfig {
    #[serde(default = "default_common_items")]
    common_items: Vec<String>,
    #[serde(default = "default_uncommon_items")]
    uncommon_items: Vec<String>,
    #[serde(default = "default_rare_items")]
    rare_items: Vec<String>,
    #[serde(default = "default_special_items")]
    special_items: Vec<String>,
}

impl Default for LootConfig {
    fn default() -> Self {
        Self {
            common_items: default_common_items(),
            uncommon_items: default_uncommon_items(),
            rare_items: default_rare_items(),
            special_items: default_special_items(),
        }
    }
}

// ============ 默认值函数 ============

fn default_true() -> bool {
    true
}
fn default_menu_id() -> String {
    "digfight".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:digfight_lobby".to_string()
}
fn default_lobby_y() -> f64 {
    -60.0
}
fn default_lobby_floor_y() -> i32 {
    -60
}
fn default_npc_z() -> f64 {
    3.5
}
fn default_npc_yaw() -> f32 {
    180.0
}
fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}
fn default_npc_display_name() -> String {
    "{\"text\":\"掘一死战\",\"color\":\"gold\"}".to_string()
}

fn default_arena_dimension() -> String {
    "qexed:digfight".to_string()
}
fn default_arena_origin_x() -> i32 {
    2048
}
fn default_arena_origin_y() -> i32 {
    -60
}
fn default_arena_origin_z() -> i32 {
    2048
}
fn default_min_players() -> i32 {
    4
}
fn default_initial_radius() -> i32 {
    12
}
fn default_min_radius() -> i32 {
    3
}
fn default_shrink_step() -> i32 {
    1
}
fn default_shrink_interval_ms() -> i64 {
    15000
}
fn default_shrink_start_delay_ms() -> i64 {
    5000
}
fn default_dig_phase_ms() -> i64 {
    60000
}
fn default_layer_count() -> i32 {
    3
}
fn default_top_layer_block() -> String {
    "minecraft:sand".to_string()
}
fn default_mid_layer_block() -> String {
    "minecraft:dirt".to_string()
}
fn default_deep_layer_block() -> String {
    "minecraft:stone".to_string()
}
fn default_special_block() -> String {
    "minecraft:diamond_block".to_string()
}

fn default_common_items() -> Vec<String> {
    vec![
        "minecraft:leather_helmet:20".to_string(),
        "minecraft:leather_chestplate:20".to_string(),
        "minecraft:leather_leggings:20".to_string(),
        "minecraft:leather_boots:20".to_string(),
        "minecraft:wooden_sword:30".to_string(),
        "minecraft:stone_sword:15".to_string(),
        "minecraft:stone_pickaxe:10".to_string(),
        "minecraft:bow:5".to_string(),
        "minecraft:arrow:20".to_string(),
        "minecraft:bread:15".to_string(),
        "minecraft:cooked_porkchop:10".to_string(),
    ]
}

fn default_uncommon_items() -> Vec<String> {
    vec![
        "minecraft:iron_helmet:20".to_string(),
        "minecraft:iron_chestplate:20".to_string(),
        "minecraft:iron_leggings:20".to_string(),
        "minecraft:iron_boots:20".to_string(),
        "minecraft:iron_sword:30".to_string(),
        "minecraft:iron_pickaxe:15".to_string(),
        "minecraft:shield:10".to_string(),
        "minecraft:bow:15".to_string(),
        "minecraft:arrow:25".to_string(),
        "minecraft:cooked_beef:15".to_string(),
        "minecraft:golden_apple:3".to_string(),
    ]
}

fn default_rare_items() -> Vec<String> {
    vec![
        "minecraft:diamond_helmet:20".to_string(),
        "minecraft:diamond_chestplate:20".to_string(),
        "minecraft:diamond_leggings:20".to_string(),
        "minecraft:diamond_boots:20".to_string(),
        "minecraft:diamond_sword:25".to_string(),
        "minecraft:diamond_pickaxe:15".to_string(),
        "minecraft:diamond_axe:10".to_string(),
        "minecraft:shield:10".to_string(),
        "minecraft:bow:15".to_string(),
        "minecraft:arrow:25".to_string(),
        "minecraft:golden_apple:10".to_string(),
        "minecraft:ender_pearl:5".to_string(),
    ]
}

fn default_special_items() -> Vec<String> {
    vec![
        "minecraft:diamond_sword:15".to_string(),
        "minecraft:diamond_chestplate:15".to_string(),
        "minecraft:netherite_sword:5".to_string(),
        "minecraft:enchanted_golden_apple:15".to_string(),
        "minecraft:golden_apple:20".to_string(),
        "minecraft:ender_pearl:15".to_string(),
        "minecraft:shield:10".to_string(),
        "minecraft:bow:10".to_string(),
        "minecraft:trident:5".to_string(),
        "minecraft:totem_of_undying:2".to_string(),
    ]
}

const DEFAULT_CONFIG: &str = r#"enable = true
menu_id = "digfight"

[lobby]
dimension = "qexed:digfight_lobby"
spawn_x = 0.5
spawn_y = -60.0
spawn_z = 0.5
spawn_yaw = 0.0
spawn_pitch = 0.0
floor_y = -60
npc_x = 0.0
npc_y = -60.0
npc_z = 3.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"
npc_display_name = "{\"text\":\"掘一死战\",\"color\":\"gold\"}"

[arena]
dimension = "qexed:digfight"
origin_x = 2048
origin_y = -60
origin_z = 2048
min_players = 4
initial_radius = 12
min_radius = 3
shrink_step = 1
shrink_interval_ms = 15000
shrink_start_delay_ms = 5000
dig_phase_ms = 60000
layer_count = 3
top_layer_block = "minecraft:sand"
mid_layer_block = "minecraft:dirt"
deep_layer_block = "minecraft:stone"
special_block = "minecraft:diamond_block"

[loot]
common_items = [
    "minecraft:leather_helmet:20",
    "minecraft:leather_chestplate:20",
    "minecraft:leather_leggings:20",
    "minecraft:leather_boots:20",
    "minecraft:wooden_sword:30",
    "minecraft:stone_sword:15",
    "minecraft:stone_pickaxe:10",
    "minecraft:bow:5",
    "minecraft:arrow:20",
    "minecraft:bread:15",
    "minecraft:cooked_porkchop:10",
]
uncommon_items = [
    "minecraft:iron_helmet:20",
    "minecraft:iron_chestplate:20",
    "minecraft:iron_leggings:20",
    "minecraft:iron_boots:20",
    "minecraft:iron_sword:30",
    "minecraft:iron_pickaxe:15",
    "minecraft:shield:10",
    "minecraft:bow:15",
    "minecraft:arrow:25",
    "minecraft:cooked_beef:15",
    "minecraft:golden_apple:3",
]
rare_items = [
    "minecraft:diamond_helmet:20",
    "minecraft:diamond_chestplate:20",
    "minecraft:diamond_leggings:20",
    "minecraft:diamond_boots:20",
    "minecraft:diamond_sword:25",
    "minecraft:diamond_pickaxe:15",
    "minecraft:diamond_axe:10",
    "minecraft:shield:10",
    "minecraft:bow:15",
    "minecraft:arrow:25",
    "minecraft:golden_apple:10",
    "minecraft:ender_pearl:5",
]
special_items = [
    "minecraft:diamond_sword:15",
    "minecraft:diamond_chestplate:15",
    "minecraft:netherite_sword:5",
    "minecraft:enchanted_golden_apple:15",
    "minecraft:golden_apple:20",
    "minecraft:ender_pearl:15",
    "minecraft:shield:10",
    "minecraft:bow:10",
    "minecraft:trident:5",
    "minecraft:totem_of_undying:2",
]
"#;