use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlayerAction, PlayerAttackQuery, PlayerAttackResponse, PlayerDeathQuery, PlayerDeathResponse,
    PlayerPayload, PlayerTickPayload, PlayerUseItemPayload, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, PluginManifest, ProjectileHitPlayerPayload,
    WorldEditRegion, config_load_or_create, config_read_to_string, storage_delete,
    storage_get_typed, storage_set_typed, time_millis, world_register_edit_region,
    world_set_blocks,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.pvp_arena".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    load_after: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const COMMAND_NAME: &str = "pvp";
const NPC_KEY: &str = "pvp.entry";
const NPC_EVENT: &str = "pvp.menu";
const SESSION_PREFIX: &str = "pvp/session/";
const PENDING_PREFIX: &str = "pvp/pending/";
const STATS_PREFIX: &str = "pvp/stats/";
const DUEL_QUEUE_KEY: &str = "pvp/queue/duel";
const BOSS_BAR_ID: &str = "pvp:status";
const PVP_ARROW_EVENT: &str = "pvp.arrow";
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    260
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let _ = storage_delete(DUEL_QUEUE_KEY);
    let config = load_config();
    register_regions(&config);
    if config.enable && config.build_maps {
        build_maps(&config);
    }
    qexed_plugin_sdk::log("pvp arena initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("pvp_arena/config.toml") || path.ends_with(CONFIG_PATH) {
        let config = load_config();
        register_regions(&config);
        if config.enable && config.build_maps {
            build_maps(&config);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    clear_player_state(&payload.uuid);
    ensure_stats(&payload.uuid, &payload.username);
    if config.enable {
        push_pending(&payload.uuid, lobby_actions(&config, "已回到 PVP 大厅"));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    remove_from_queue(&payload.uuid);
    finish_session_on_leave(&config, &payload.uuid, &payload.username);
    clear_player_state(&payload.uuid);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "PVP 竞技".to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PluginCommandQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.command != COMMAND_NAME {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let config = load_config();
    let response = handle_command(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_mutations(_ptr: i32, _len: i32) -> i64 {
    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse::default());
    }

    qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse {
        operations: vec![NpcMutationOp::Upsert {
            npc: NpcUpsert {
                key: NPC_KEY.to_string(),
                dimension: config.lobby.dimension.clone(),
                x: config.lobby.npc_x,
                y: config.lobby.npc_y,
                z: config.lobby.npc_z,
                yaw: config.lobby.npc_yaw,
                pitch: config.lobby.npc_pitch,
                name: "PVP 竞技".to_string(),
                display_name: config.lobby.npc_display_name.clone(),
                entity_type: config.lobby.npc_entity_type.clone(),
                skin_textures: String::new(),
                skin_signature: String::new(),
                look_at_players: true,
                main_hand_event: NPC_EVENT.to_string(),
                off_hand_event: NPC_EVENT.to_string(),
                attack_event: NPC_EVENT.to_string(),
            },
        }],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<NpcInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.entity.key != NPC_KEY || payload.configured_event != NPC_EVENT {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let config = load_config();
    qexed_plugin_sdk::response_ptr_len(&open_menu(&config))
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_player_tick(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_attack(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerAttackQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerAttackResponse::default());
    };
    let config = load_config();
    let response = handle_player_attack(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_use_item(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerUseItemPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_player_use_item(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_projectile_hit_player(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ProjectileHitPlayerPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    let response = handle_projectile_hit_player(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_death(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerDeathQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerDeathResponse::default());
    };
    let config = load_config();
    let response = handle_player_death(&config, &payload);
    qexed_plugin_sdk::response_ptr_len(&response)
}

fn handle_command(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if !config.enable {
        return handled(vec![message("PVP 插件未启用")]);
    }

    let args = payload
        .argument
        .split_whitespace()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        None | Some("") | Some("menu") => open_menu(config),
        Some("duel") | Some("match") | Some("1v1") => join_duel_queue(config, payload),
        Some("join")
            if args
                .get(1)
                .is_some_and(|mode| mode == "1v1" || mode == "duel") =>
        {
            join_duel_queue(config, payload)
        }
        Some("ffa") => join_ffa(config, payload),
        Some("leave") | Some("lobby") | Some("quit") => leave_game(config, &payload.player.uuid),
        Some("server") | Some("hub") => leave_to_server_lobby(config, &payload.player.uuid),
        Some("stats") => show_stats(&payload.player.uuid),
        Some("status") => show_status(&payload.player.uuid),
        _ => handled(vec![message(
            "用法: /pvp, /pvp duel, /pvp ffa, /pvp leave, /pvp server, /pvp stats",
        )]),
    }
}

fn open_menu(config: &Config) -> PluginCommandResponse {
    handled(vec![PlayerAction::OpenMenu {
        menu: config.menu_id.clone(),
    }])
}

fn join_duel_queue(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    if let Some(session) = get_session(&payload.player.uuid) {
        return handled(vec![message(format!(
            "你已经在 {} 中",
            mode_label(&session.mode)
        ))]);
    }

    let mut queue = get_duel_queue();
    queue
        .players
        .retain(|entry| entry.uuid != payload.player.uuid);
    if let Some(opponent) = queue.players.pop() {
        set_duel_queue(&queue);
        start_duel(config, payload, opponent)
    } else {
        queue.players.push(QueueEntry {
            uuid: payload.player.uuid.clone(),
            username: payload.player.username.clone(),
            joined_at_ms: time_millis(),
        });
        set_duel_queue(&queue);
        handled(vec![
            message("已进入 1v1 匹配队列"),
            PlayerAction::BossBar {
                id: BOSS_BAR_ID.to_string(),
                title: "PVP | 1v1 匹配中".to_string(),
                progress: 1.0,
                color: "yellow".to_string(),
                overlay: "progress".to_string(),
            },
        ])
    }
}

fn start_duel(
    config: &Config,
    payload: &PluginCommandQuery,
    opponent: QueueEntry,
) -> PluginCommandResponse {
    let game_id = format!("duel_{}", time_millis());
    let player = QueueEntry {
        uuid: payload.player.uuid.clone(),
        username: payload.player.username.clone(),
        joined_at_ms: time_millis(),
    };
    save_session(
        &player.uuid,
        &PlayerSession {
            mode: GameModeKind::Duel,
            game_id: game_id.clone(),
            opponent_uuid: opponent.uuid.clone(),
            opponent_name: opponent.username.clone(),
            alive: true,
            last_shot_ms: 0,
        },
    );
    save_session(
        &opponent.uuid,
        &PlayerSession {
            mode: GameModeKind::Duel,
            game_id,
            opponent_uuid: player.uuid.clone(),
            opponent_name: player.username.clone(),
            alive: true,
            last_shot_ms: 0,
        },
    );

    push_pending(
        &opponent.uuid,
        duel_start_actions(config, true, &player.username),
    );
    handled(duel_start_actions(config, false, &opponent.username))
}

fn join_ffa(config: &Config, payload: &PluginCommandQuery) -> PluginCommandResponse {
    remove_from_queue(&payload.player.uuid);
    save_session(
        &payload.player.uuid,
        &PlayerSession {
            mode: GameModeKind::Ffa,
            game_id: "ffa".to_string(),
            opponent_uuid: String::new(),
            opponent_name: String::new(),
            alive: true,
            last_shot_ms: 0,
        },
    );
    handled(ffa_spawn_actions(config, "已进入 FFA 竞技场"))
}

fn leave_game(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    remove_from_queue(player_uuid);
    if let Some(session) = get_session(player_uuid) {
        if session.mode == GameModeKind::Duel && !session.opponent_uuid.is_empty() {
            let name = get_stats(player_uuid)
                .map(|stats| stats.username)
                .unwrap_or_else(|| "对手".to_string());
            award_duel_win(
                config,
                &session.opponent_uuid,
                player_uuid,
                format!("{name} 退出了对局"),
            );
        }
    }
    clear_player_state(player_uuid);
    handled(lobby_actions(config, "已返回 PVP 大厅"))
}

fn leave_to_server_lobby(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    let mut response = leave_game(config, player_uuid);
    response.actions.push(PlayerAction::ProxyConnect {
        server: config.server_lobby_id.clone(),
        message: "正在返回服务器大厅".to_string(),
    });
    response
}

fn show_status(player_uuid: &str) -> PluginCommandResponse {
    if let Some(session) = get_session(player_uuid) {
        return handled(vec![message(format!(
            "当前模式: {}，对手: {}",
            mode_label(&session.mode),
            if session.opponent_name.is_empty() {
                "无"
            } else {
                &session.opponent_name
            }
        ))]);
    }
    let queue = get_duel_queue();
    if queue.players.iter().any(|entry| entry.uuid == player_uuid) {
        return handled(vec![message(format!(
            "当前在 1v1 队列中，队列人数: {}",
            queue.players.len()
        ))]);
    }
    handled(vec![message("当前不在 PVP 对局中")])
}

fn show_stats(player_uuid: &str) -> PluginCommandResponse {
    let stats = get_stats(player_uuid).unwrap_or_default();
    handled(vec![message(format!(
        "PVP 数据 | 胜: {} 负: {} 击杀: {} 死亡: {}",
        stats.wins, stats.losses, stats.kills, stats.deaths
    ))])
}

fn handle_player_tick(config: &Config, payload: &PlayerTickPayload) -> PluginCommandResponse {
    let mut actions = take_pending(&payload.player.uuid);
    if let Some(session) = get_session(&payload.player.uuid) {
        actions.push(PlayerAction::BossBar {
            id: BOSS_BAR_ID.to_string(),
            title: status_bar_title(&session),
            progress: 1.0,
            color: if session.mode == GameModeKind::Duel {
                "red".to_string()
            } else {
                "green".to_string()
            },
            overlay: "progress".to_string(),
        });
    } else if actions.is_empty() {
        return PluginCommandResponse::default();
    }

    if !config.enable {
        actions.retain(|action| !matches!(action, PlayerAction::BossBar { .. }));
    }
    handled(actions)
}

fn handle_player_attack(config: &Config, payload: &PlayerAttackQuery) -> PlayerAttackResponse {
    if !config.enable {
        return PlayerAttackResponse::default();
    }
    let Some(target) = &payload.target_player else {
        return PlayerAttackResponse::default();
    };
    let Some(attacker_session) = get_session(&payload.player.uuid) else {
        if get_session(&target.uuid).is_some() {
            return PlayerAttackResponse {
                cancel: true,
                ..PlayerAttackResponse::default()
            };
        }
        return PlayerAttackResponse::default();
    };
    let Some(target_session) = get_session(&target.uuid) else {
        return PlayerAttackResponse {
            cancel: true,
            ..PlayerAttackResponse::default()
        };
    };
    if !can_damage(
        &attacker_session,
        &target_session,
        &payload.player.uuid,
        &target.uuid,
    ) {
        return PlayerAttackResponse {
            cancel: true,
            ..PlayerAttackResponse::default()
        };
    }

    let amount = attack_damage(config, &payload.weapon.item_name, payload.damage);
    PlayerAttackResponse {
        cancel: true,
        damage: Some(0.0),
        actions: vec![PlayerAction::DamagePlayer {
            uuid: target.uuid.clone(),
            username: String::new(),
            amount,
            kind: "player_attack".to_string(),
            source_entity_id: payload.player.entity_id,
            source_x: payload.position.x,
            source_y: payload.position.y,
            source_z: payload.position.z,
            knockback: config.combat.knockback,
        }],
        ..PlayerAttackResponse::default()
    }
}

fn handle_player_use_item(
    config: &Config,
    payload: &PlayerUseItemPayload,
) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }
    if payload.action == "use_item" && is_lobby_return_item(config, payload) {
        return leave_game(config, &payload.player.uuid);
    }
    if payload.action == "use_item" && is_server_lobby_return_item(config, payload) {
        return leave_to_server_lobby(config, &payload.player.uuid);
    }
    if payload.action != "release_use_item" {
        return PluginCommandResponse::default();
    }
    let Some(mut session) = get_session(&payload.player.uuid) else {
        return PluginCommandResponse::default();
    };
    if !session.alive || !is_pvp_bow(config, payload) {
        return PluginCommandResponse::default();
    }
    let now = time_millis();
    if now.saturating_sub(session.last_shot_ms) < config.combat.bow_cooldown_ms {
        return handled(vec![message("PVP 弓冷却中")]);
    }
    session.last_shot_ms = now;
    save_session(&payload.player.uuid, &session);
    handled(vec![pvp_arrow_projectile(payload)])
}

fn handle_projectile_hit_player(
    config: &Config,
    payload: &ProjectileHitPlayerPayload,
) -> PluginCommandResponse {
    if !config.enable {
        return PluginCommandResponse::default();
    }
    if payload.configured_event != PVP_ARROW_EVENT || payload.projectile_kind != "arrow" {
        return PluginCommandResponse::default();
    }
    if !payload.tag.trim().ends_with(config.kit.bow_name.trim()) {
        return PluginCommandResponse::default();
    }
    let Some(shooter_session) = get_session(&payload.shooter.uuid) else {
        return PluginCommandResponse::default();
    };
    let Some(target_session) = get_session(&payload.target.uuid) else {
        return PluginCommandResponse::default();
    };
    if !can_damage(
        &shooter_session,
        &target_session,
        &payload.shooter.uuid,
        &payload.target.uuid,
    ) {
        return PluginCommandResponse::default();
    }

    handled(vec![PlayerAction::DamagePlayer {
        uuid: payload.target.uuid.clone(),
        username: String::new(),
        amount: config.combat.arrow_damage.max(0.0),
        kind: "projectile".to_string(),
        source_entity_id: payload.shooter.entity_id,
        source_x: payload.position.x,
        source_y: payload.position.y,
        source_z: payload.position.z,
        knockback: config.combat.arrow_knockback.max(0.0),
    }])
}

fn handle_player_death(config: &Config, payload: &PlayerDeathQuery) -> PlayerDeathResponse {
    let Some(session) = get_session(&payload.player.uuid) else {
        return PlayerDeathResponse::default();
    };
    if session.mode == GameModeKind::Duel {
        let loser_uuid = payload.player.uuid.clone();
        let loser_name = payload.player.username.clone();
        let winner_uuid = session.opponent_uuid.clone();
        award_duel_win(
            config,
            &winner_uuid,
            &loser_uuid,
            format!("{loser_name} 被击败"),
        );
        return PlayerDeathResponse {
            cancel: true,
            message: "你已被击败，正在返回大厅".to_string(),
            overlay: false,
        };
    }

    add_death(&payload.player.uuid, &payload.player.username);
    clear_player_state(&payload.player.uuid);
    save_session(
        &payload.player.uuid,
        &PlayerSession {
            mode: GameModeKind::Ffa,
            game_id: "ffa".to_string(),
            opponent_uuid: String::new(),
            opponent_name: String::new(),
            alive: true,
            last_shot_ms: 0,
        },
    );
    push_pending(
        &payload.player.uuid,
        ffa_spawn_actions(config, "你已阵亡，已重新进入 FFA"),
    );
    PlayerDeathResponse {
        cancel: true,
        message: "你已阵亡，正在重生".to_string(),
        overlay: false,
    }
}

fn can_damage(
    attacker_session: &PlayerSession,
    target_session: &PlayerSession,
    attacker_uuid: &str,
    target_uuid: &str,
) -> bool {
    if attacker_uuid == target_uuid || !attacker_session.alive || !target_session.alive {
        return false;
    }
    match (&attacker_session.mode, &target_session.mode) {
        (GameModeKind::Duel, GameModeKind::Duel) => {
            attacker_session.game_id == target_session.game_id
                && attacker_session.opponent_uuid == target_uuid
                && target_session.opponent_uuid == attacker_uuid
        }
        (GameModeKind::Ffa, GameModeKind::Ffa) => true,
        _ => false,
    }
}

fn attack_damage(config: &Config, weapon: &str, fallback: f32) -> f32 {
    let weapon = weapon.trim();
    if weapon.ends_with("_axe") {
        config.combat.axe_damage.max(0.0)
    } else if weapon.ends_with("_sword") {
        config.combat.sword_damage.max(0.0)
    } else {
        config.combat.fist_damage.max(fallback.max(0.0))
    }
}

fn award_duel_win(config: &Config, winner_uuid: &str, loser_uuid: &str, reason: String) {
    if winner_uuid.is_empty() {
        clear_player_state(loser_uuid);
        return;
    }

    let winner_name = get_stats(winner_uuid)
        .map(|stats| stats.username)
        .unwrap_or_else(|| "胜者".to_string());
    let loser_name = get_stats(loser_uuid)
        .map(|stats| stats.username)
        .unwrap_or_else(|| "败者".to_string());

    add_win(winner_uuid, &winner_name);
    add_loss(loser_uuid, &loser_name);
    clear_player_state(winner_uuid);
    clear_player_state(loser_uuid);

    push_pending(
        winner_uuid,
        lobby_actions(config, format!("{reason}，你赢得了 1v1")),
    );
    push_pending(
        loser_uuid,
        lobby_actions(config, format!("你输给了 {winner_name}")),
    );
}

fn finish_session_on_leave(config: &Config, player_uuid: &str, username: &str) {
    if let Some(session) = get_session(player_uuid) {
        if session.mode == GameModeKind::Duel && !session.opponent_uuid.is_empty() {
            award_duel_win(
                config,
                &session.opponent_uuid,
                player_uuid,
                format!("{username} 离线了"),
            );
        }
    }
}

fn duel_start_actions(config: &Config, spawn_a: bool, opponent: &str) -> Vec<PlayerAction> {
    let spawn = if spawn_a {
        &config.duel.spawn_a
    } else {
        &config.duel.spawn_b
    };
    let mut actions = vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.duel.dimension.clone(),
            x: spawn.x,
            y: spawn.y,
            z: spawn.z,
            yaw: Some(spawn.yaw),
            pitch: Some(spawn.pitch),
        },
        PlayerAction::ResetInventory {
            restore_menu_items: false,
        },
        message(format!("1v1 开始！对手: {opponent}")),
    ];
    actions.extend(kit_actions(config));
    actions
}

fn ffa_spawn_actions(config: &Config, text: impl Into<String>) -> Vec<PlayerAction> {
    let mut actions = vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.ffa.dimension.clone(),
            x: config.ffa.spawn.x,
            y: config.ffa.spawn.y,
            z: config.ffa.spawn.z,
            yaw: Some(config.ffa.spawn.yaw),
            pitch: Some(config.ffa.spawn.pitch),
        },
        PlayerAction::ResetInventory {
            restore_menu_items: false,
        },
        message(text.into()),
    ];
    actions.extend(kit_actions(config));
    actions
}

fn lobby_actions(config: &Config, text: impl Into<String>) -> Vec<PlayerAction> {
    vec![
        PlayerAction::SetPlayersVisible { visible: true },
        PlayerAction::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        PlayerAction::ResetInventory {
            restore_menu_items: true,
        },
        PlayerAction::RemoveBossBar {
            id: BOSS_BAR_ID.to_string(),
        },
        message(text.into()),
    ]
}

fn kit_actions(config: &Config) -> Vec<PlayerAction> {
    let mut actions = vec![
        PlayerAction::GiveItem {
            item: config.kit.sword.clone(),
            count: 1,
            name: config.kit.sword_name.clone(),
            lore: Vec::new(),
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        },
        PlayerAction::GiveItem {
            item: config.kit.bow.clone(),
            count: 1,
            name: config.kit.bow_name.clone(),
            lore: Vec::new(),
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        },
        PlayerAction::GiveItem {
            item: "minecraft:arrow".to_string(),
            count: config.kit.arrows.clamp(1, 64),
            name: String::new(),
            lore: Vec::new(),
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        },
        PlayerAction::GiveItem {
            item: config.kit.food.clone(),
            count: config.kit.food_count.clamp(1, 64),
            name: String::new(),
            lore: Vec::new(),
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        },
    ];
    if config.kit.golden_apples > 0 {
        actions.push(PlayerAction::GiveItem {
            item: config.kit.golden_apple_item.clone(),
            count: config.kit.golden_apples.clamp(1, 64),
            name: String::new(),
            lore: Vec::new(),
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        });
    }
    actions.push(PlayerAction::GiveItem {
        item: config.kit.lobby_item.clone(),
        count: 1,
        name: config.kit.lobby_item_name.clone(),
        lore: vec!["右键返回 PVP 大厅".to_string()],
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
    });
    actions.push(PlayerAction::GiveItem {
        item: config.kit.server_lobby_item.clone(),
        count: 1,
        name: config.kit.server_lobby_item_name.clone(),
        lore: vec!["右键返回浅屿闲游大厅".to_string()],
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
    });
    actions
}

fn is_lobby_return_item(config: &Config, payload: &PlayerUseItemPayload) -> bool {
    payload.item.item_name == config.kit.lobby_item
        && payload
            .item
            .display_name
            .trim()
            .ends_with(config.kit.lobby_item_name.trim())
}

fn is_server_lobby_return_item(config: &Config, payload: &PlayerUseItemPayload) -> bool {
    payload.item.item_name == config.kit.server_lobby_item
        && payload
            .item
            .display_name
            .trim()
            .ends_with(config.kit.server_lobby_item_name.trim())
}

fn is_pvp_bow(config: &Config, payload: &PlayerUseItemPayload) -> bool {
    payload.item.item_name == config.kit.bow
        && payload
            .item
            .display_name
            .trim()
            .ends_with(config.kit.bow_name.trim())
}

fn pvp_arrow_projectile(payload: &PlayerUseItemPayload) -> PlayerAction {
    let (dir_x, dir_y, dir_z) = look_direction(payload.yaw, payload.pitch);
    let speed = 1.9;
    PlayerAction::SpawnProjectile {
        kind: "arrow".to_string(),
        configured_event: PVP_ARROW_EVENT.to_string(),
        tag: payload.item.display_name.clone(),
        dimension: payload.dimension.clone(),
        x: payload.position.x + dir_x * 0.6,
        y: payload.position.y + 1.55 + dir_y * 0.6,
        z: payload.position.z + dir_z * 0.6,
        velocity_x: dir_x * speed,
        velocity_y: dir_y * speed,
        velocity_z: dir_z * speed,
        source_entity_id: payload.player.entity_id,
        damage: 0.0,
        knockback: 0.0,
        gravity_per_tick: 0.03,
        hit_radius: 0.25,
        lifetime_ticks: 60,
    }
}

fn look_direction(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw = f64::from(yaw).to_radians();
    let pitch = f64::from(pitch).to_radians();
    (
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

fn register_regions(config: &Config) {
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "pvp_lobby",
        dimension: &config.lobby.dimension,
        min: (-24, config.lobby.floor_y - 4, -24),
        max: (24, config.lobby.floor_y + 16, 24),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "pvp_duel",
        dimension: &config.duel.dimension,
        min: (-40, config.duel.floor_y - 4, -40),
        max: (40, config.duel.floor_y + 24, 40),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "pvp_ffa",
        dimension: &config.ffa.dimension,
        min: (-56, config.ffa.floor_y - 4, -56),
        max: (56, config.ffa.floor_y + 24, 56),
        allow_player_break: false,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn build_maps(config: &Config) {
    build_platform(
        &config.lobby.dimension,
        config.lobby.floor_y,
        config.lobby.radius,
        "minecraft:smooth_quartz",
        "minecraft:sea_lantern",
    );
    build_platform(
        &config.duel.dimension,
        config.duel.floor_y,
        config.duel.radius,
        "minecraft:polished_deepslate",
        "minecraft:redstone_lamp",
    );
    build_platform(
        &config.ffa.dimension,
        config.ffa.floor_y,
        config.ffa.radius,
        "minecraft:stone_bricks",
        "minecraft:glowstone",
    );
}

fn build_platform(dimension: &str, floor_y: i32, radius: i32, floor: &str, marker: &str) {
    let radius = radius.clamp(4, 64);
    let mut blocks = Vec::new();
    for x in -radius..=radius {
        for z in -radius..=radius {
            blocks.push((dimension, (x, floor_y, z), floor));
            if x.abs() == radius || z.abs() == radius {
                blocks.push((dimension, (x, floor_y + 1, z), "minecraft:glass_pane"));
            }
            if blocks.len() >= WORLD_EDIT_BATCH_LIMIT {
                let _ = world_set_blocks(blocks.drain(..));
            }
        }
    }
    for (x, z) in [
        (-radius, -radius),
        (radius, -radius),
        (-radius, radius),
        (radius, radius),
    ] {
        blocks.push((dimension, (x, floor_y + 1, z), marker));
    }
    if !blocks.is_empty() {
        let _ = world_set_blocks(blocks);
    }
}

fn clear_player_state(player_uuid: &str) {
    let _ = storage_delete(&session_key(player_uuid));
    let _ = storage_delete(&pending_key(player_uuid));
}

fn remove_from_queue(player_uuid: &str) {
    let mut queue = get_duel_queue();
    queue.players.retain(|entry| entry.uuid != player_uuid);
    set_duel_queue(&queue);
}

fn get_session(player_uuid: &str) -> Option<PlayerSession> {
    storage_get_typed(&session_key(player_uuid))
}

fn save_session(player_uuid: &str, session: &PlayerSession) {
    let _ = storage_set_typed(&session_key(player_uuid), session);
}

fn get_duel_queue() -> DuelQueue {
    storage_get_typed(DUEL_QUEUE_KEY).unwrap_or_default()
}

fn set_duel_queue(queue: &DuelQueue) {
    let _ = storage_set_typed(DUEL_QUEUE_KEY, queue);
}

fn push_pending(player_uuid: &str, mut actions: Vec<PlayerAction>) {
    let key = pending_key(player_uuid);
    let mut pending = storage_get_typed::<PendingActions>(&key).unwrap_or_default();
    pending.actions.append(&mut actions);
    let _ = storage_set_typed(&key, &pending);
}

fn take_pending(player_uuid: &str) -> Vec<PlayerAction> {
    let key = pending_key(player_uuid);
    let pending = storage_get_typed::<PendingActions>(&key).unwrap_or_default();
    let _ = storage_delete(&key);
    pending.actions
}

fn ensure_stats(player_uuid: &str, username: &str) {
    let mut stats = get_stats(player_uuid).unwrap_or_default();
    if !username.trim().is_empty() {
        stats.username = username.to_string();
    }
    let _ = storage_set_typed(&stats_key(player_uuid), &stats);
}

fn add_win(player_uuid: &str, username: &str) {
    let mut stats = get_stats(player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    stats.wins = stats.wins.saturating_add(1);
    stats.kills = stats.kills.saturating_add(1);
    let _ = storage_set_typed(&stats_key(player_uuid), &stats);
}

fn add_loss(player_uuid: &str, username: &str) {
    let mut stats = get_stats(player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    stats.losses = stats.losses.saturating_add(1);
    stats.deaths = stats.deaths.saturating_add(1);
    let _ = storage_set_typed(&stats_key(player_uuid), &stats);
}

fn add_death(player_uuid: &str, username: &str) {
    let mut stats = get_stats(player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    stats.deaths = stats.deaths.saturating_add(1);
    let _ = storage_set_typed(&stats_key(player_uuid), &stats);
}

fn get_stats(player_uuid: &str) -> Option<PlayerStats> {
    storage_get_typed(&stats_key(player_uuid))
}

fn status_bar_title(session: &PlayerSession) -> String {
    match session.mode {
        GameModeKind::Duel => format!("PVP | 1v1 | 对手 {}", session.opponent_name),
        GameModeKind::Ffa => "PVP | FFA".to_string(),
    }
}

fn mode_label(mode: &GameModeKind) -> &'static str {
    match mode {
        GameModeKind::Duel => "1v1 对决",
        GameModeKind::Ffa => "FFA 乱斗",
    }
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{player_uuid}")
}

fn pending_key(player_uuid: &str) -> String {
    format!("{PENDING_PREFIX}{player_uuid}")
}

fn stats_key(player_uuid: &str) -> String {
    format!("{STATS_PREFIX}{player_uuid}")
}

fn handled(actions: Vec<PlayerAction>) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions,
    }
}

fn message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|contents| toml::from_str::<Config>(&contents).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default pvp_arena config is valid")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSession {
    mode: GameModeKind,
    game_id: String,
    opponent_uuid: String,
    opponent_name: String,
    alive: bool,
    #[serde(default)]
    last_shot_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum GameModeKind {
    Duel,
    Ffa,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DuelQueue {
    players: Vec<QueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueEntry {
    uuid: String,
    username: String,
    joined_at_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PendingActions {
    actions: Vec<PlayerAction>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PlayerStats {
    username: String,
    wins: u32,
    losses: u32,
    kills: u32,
    deaths: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_true")]
    build_maps: bool,
    #[serde(default = "default_menu_id")]
    menu_id: String,
    #[serde(default)]
    lobby: LobbyConfig,
    #[serde(default)]
    duel: ArenaConfig,
    #[serde(default)]
    ffa: ArenaConfig,
    #[serde(default)]
    combat: CombatConfig,
    #[serde(default)]
    kit: KitConfig,
    #[serde(default = "default_server_lobby_id")]
    server_lobby_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LobbyConfig {
    #[serde(default = "default_lobby_dimension")]
    dimension: String,
    #[serde(default = "default_spawn_x")]
    spawn_x: f64,
    #[serde(default = "default_spawn_y")]
    spawn_y: f64,
    #[serde(default = "default_spawn_z")]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "default_floor_y")]
    floor_y: i32,
    #[serde(default = "default_lobby_radius")]
    radius: i32,
    #[serde(default = "default_npc_x")]
    npc_x: f64,
    #[serde(default = "default_npc_y")]
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
            spawn_x: default_spawn_x(),
            spawn_y: default_spawn_y(),
            spawn_z: default_spawn_z(),
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: default_floor_y(),
            radius: default_lobby_radius(),
            npc_x: default_npc_x(),
            npc_y: default_npc_y(),
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
    #[serde(default = "default_duel_dimension")]
    dimension: String,
    #[serde(default = "default_floor_y")]
    floor_y: i32,
    #[serde(default = "default_arena_radius")]
    radius: i32,
    #[serde(default)]
    spawn: SpawnConfig,
    #[serde(default = "default_duel_spawn_a")]
    spawn_a: SpawnConfig,
    #[serde(default = "default_duel_spawn_b")]
    spawn_b: SpawnConfig,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            dimension: default_duel_dimension(),
            floor_y: default_floor_y(),
            radius: default_arena_radius(),
            spawn: SpawnConfig::default(),
            spawn_a: default_duel_spawn_a(),
            spawn_b: default_duel_spawn_b(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SpawnConfig {
    #[serde(default = "default_spawn_x")]
    x: f64,
    #[serde(default = "default_spawn_y")]
    y: f64,
    #[serde(default = "default_spawn_z")]
    z: f64,
    #[serde(default)]
    yaw: f32,
    #[serde(default)]
    pitch: f32,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            x: default_spawn_x(),
            y: default_spawn_y(),
            z: default_spawn_z(),
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CombatConfig {
    #[serde(default = "default_sword_damage")]
    sword_damage: f32,
    #[serde(default = "default_axe_damage")]
    axe_damage: f32,
    #[serde(default = "default_fist_damage")]
    fist_damage: f32,
    #[serde(default = "default_arrow_damage")]
    arrow_damage: f32,
    #[serde(default = "default_knockback")]
    knockback: f32,
    #[serde(default = "default_arrow_knockback")]
    arrow_knockback: f32,
    #[serde(default = "default_bow_cooldown_ms")]
    bow_cooldown_ms: i64,
}

impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            sword_damage: default_sword_damage(),
            axe_damage: default_axe_damage(),
            fist_damage: default_fist_damage(),
            arrow_damage: default_arrow_damage(),
            knockback: default_knockback(),
            arrow_knockback: default_arrow_knockback(),
            bow_cooldown_ms: default_bow_cooldown_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct KitConfig {
    #[serde(default = "default_sword")]
    sword: String,
    #[serde(default = "default_sword_name")]
    sword_name: String,
    #[serde(default = "default_bow")]
    bow: String,
    #[serde(default = "default_bow_name")]
    bow_name: String,
    #[serde(default = "default_arrows")]
    arrows: i32,
    #[serde(default = "default_food")]
    food: String,
    #[serde(default = "default_food_count")]
    food_count: i32,
    #[serde(default = "default_golden_apple_item")]
    golden_apple_item: String,
    #[serde(default = "default_golden_apples")]
    golden_apples: i32,
    #[serde(default = "default_lobby_item")]
    lobby_item: String,
    #[serde(default = "default_lobby_item_name")]
    lobby_item_name: String,
    #[serde(default = "default_server_lobby_item")]
    server_lobby_item: String,
    #[serde(default = "default_server_lobby_item_name")]
    server_lobby_item_name: String,
}

impl Default for KitConfig {
    fn default() -> Self {
        Self {
            sword: default_sword(),
            sword_name: default_sword_name(),
            bow: default_bow(),
            bow_name: default_bow_name(),
            arrows: default_arrows(),
            food: default_food(),
            food_count: default_food_count(),
            golden_apple_item: default_golden_apple_item(),
            golden_apples: default_golden_apples(),
            lobby_item: default_lobby_item(),
            lobby_item_name: default_lobby_item_name(),
            server_lobby_item: default_server_lobby_item(),
            server_lobby_item_name: default_server_lobby_item_name(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_menu_id() -> String {
    "pvp".to_string()
}

fn default_server_lobby_id() -> String {
    "lobby_1".to_string()
}

fn default_lobby_dimension() -> String {
    "qexed:pvp_lobby".to_string()
}

fn default_duel_dimension() -> String {
    "qexed:pvp_duel".to_string()
}

fn default_spawn_x() -> f64 {
    0.5
}

fn default_spawn_y() -> f64 {
    80.0
}

fn default_spawn_z() -> f64 {
    0.5
}

fn default_floor_y() -> i32 {
    80
}

fn default_lobby_radius() -> i32 {
    12
}

fn default_arena_radius() -> i32 {
    24
}

fn default_npc_x() -> f64 {
    2.5
}

fn default_npc_y() -> f64 {
    81.0
}

fn default_npc_z() -> f64 {
    2.5
}

fn default_npc_yaw() -> f32 {
    180.0
}

fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_npc_display_name() -> String {
    "{\"text\":\"PVP 竞技\",\"color\":\"red\"}".to_string()
}

fn default_duel_spawn_a() -> SpawnConfig {
    SpawnConfig {
        x: -8.5,
        y: 81.0,
        z: 0.5,
        yaw: -90.0,
        pitch: 0.0,
    }
}

fn default_duel_spawn_b() -> SpawnConfig {
    SpawnConfig {
        x: 8.5,
        y: 81.0,
        z: 0.5,
        yaw: 90.0,
        pitch: 0.0,
    }
}

fn default_sword_damage() -> f32 {
    6.0
}

fn default_axe_damage() -> f32 {
    7.0
}

fn default_fist_damage() -> f32 {
    1.0
}

fn default_arrow_damage() -> f32 {
    3.0
}

fn default_knockback() -> f32 {
    0.42
}

fn default_arrow_knockback() -> f32 {
    0.55
}

fn default_bow_cooldown_ms() -> i64 {
    900
}

fn default_sword() -> String {
    "minecraft:iron_sword".to_string()
}

fn default_sword_name() -> String {
    "PVP 铁剑".to_string()
}

fn default_bow() -> String {
    "minecraft:bow".to_string()
}

fn default_bow_name() -> String {
    "PVP 弓".to_string()
}

fn default_arrows() -> i32 {
    32
}

fn default_food() -> String {
    "minecraft:cooked_beef".to_string()
}

fn default_food_count() -> i32 {
    16
}

fn default_golden_apples() -> i32 {
    2
}

fn default_golden_apple_item() -> String {
    "minecraft:enchanted_golden_apple".to_string()
}

fn default_lobby_item() -> String {
    "minecraft:barrier".to_string()
}

fn default_lobby_item_name() -> String {
    "返回 PVP 大厅".to_string()
}

fn default_server_lobby_item() -> String {
    "minecraft:ender_pearl".to_string()
}

fn default_server_lobby_item_name() -> String {
    "返回服务器大厅".to_string()
}

const DEFAULT_CONFIG: &str = r#"enable = true
build_maps = true
menu_id = "pvp"
server_lobby_id = "lobby_1"

[lobby]
dimension = "qexed:pvp_lobby"
spawn_x = 0.5
spawn_y = 81.0
spawn_z = 0.5
spawn_yaw = 0.0
spawn_pitch = 0.0
floor_y = 80
radius = 12
npc_x = 2.5
npc_y = 81.0
npc_z = 2.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:villager"
npc_display_name = "{\"text\":\"PVP 竞技\",\"color\":\"red\"}"

[duel]
dimension = "qexed:pvp_duel"
floor_y = 80
radius = 24

[duel.spawn_a]
x = -8.5
y = 81.0
z = 0.5
yaw = -90.0
pitch = 0.0

[duel.spawn_b]
x = 8.5
y = 81.0
z = 0.5
yaw = 90.0
pitch = 0.0

[ffa]
dimension = "qexed:pvp_ffa"
floor_y = 80
radius = 28

[ffa.spawn]
x = 0.5
y = 81.0
z = 0.5
yaw = 0.0
pitch = 0.0

[combat]
sword_damage = 6.0
axe_damage = 7.0
fist_damage = 1.0
arrow_damage = 3.0
knockback = 0.42
arrow_knockback = 0.55
bow_cooldown_ms = 900

[kit]
sword = "minecraft:iron_sword"
sword_name = "PVP 铁剑"
bow = "minecraft:bow"
bow_name = "PVP 弓"
arrows = 32
food = "minecraft:cooked_beef"
food_count = 16
golden_apple_item = "minecraft:enchanted_golden_apple"
golden_apples = 2
lobby_item = "minecraft:barrier"
lobby_item_name = "返回 PVP 大厅"
server_lobby_item = "minecraft:ender_pearl"
server_lobby_item_name = "返回服务器大厅"
"#;
