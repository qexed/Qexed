//! pvp_arena：v4 WASM 示例插件 → v6 原生 cdylib 插件移植。
//!
//! v4 导出 → v6 事件映射：
//! - init                     → declare_simple_plugin! init 闭包（清队列/登记编辑区/建场）
//! - config_reload            → ConfigReloadEvent（宿主未接线，休眠）
//! - player_join              → PlayerJoinEvent（清状态 + ensure_stats + 回大厅动作即时下发）
//! - player_leave             → PlayerLeaveEvent（离队 + duel 对手判胜）
//! - commands（命令清单）      → ctx.register_command("pvp")
//! - command_execute          → ChatCommandEvent（cancel 接管）
//! - npc_mutations            → NpcMutationsEvent（Mutex 回写；宿主未接线，休眠）
//! - npc_interact             → NpcInteractEvent
//! - player_tick              → PlayerTickEvent（休眠；v4 pending 下发改为即时下发）
//! - player_attack            → PlayerAttackEvent（v6 无 target_player/伤害改写字段，见 handler）
//! - player_use_item          → PlayerUseItemEvent
//! - projectile_hit_player    → ProjectileHitPlayerEvent（宿主无弹射物运行时，休眠）
//! - player_death             → PlayerDeathEvent（宿主未接线，休眠）
//!
//! 语义差异（相对 v4）：
//! 1. v4 用存储键 pvp/pending/<uuid> 暂存动作、player_tick 时批量下发（WASM handler
//!    无法即时下发）；v6 handler 内可直接 sdk.perform，pending 表取消，动作即时下发。
//! 2. v4 存储键/动作 id pvp:status、pvp.entry 含冒号（v6 存储键字符集不允许），改为
//!    pvp_status / pvp_entry；NPC 事件名 pvp.menu 同步改 pvp_menu。
//! 3. v6 PlayerAttackEvent 无 target_player / damage 改写 / actions 响应字段，无法在
//!    攻击事件里按目标玩家判局并下发自定义伤害；该 handler 仅占位，完整语义待宿主
//!    载荷补齐后恢复。
//! 4. SpawnProjectile/DamagePlayer/ResetInventory/SetPlayersVisible/OpenMenu 宿主 v6
//!    降级（无弹射物运行时/无背包副本/无菜单基础设施），动作照发，行为随宿主升级恢复。
//! 5. v4 priority() = 260 丢弃：v6 事件按注册顺序（FIFO）分发。

use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use qexed_plugin_api::payload::{
    ItemStackPayload, NpcMutationOp, NpcUpsert, PlayerAction as Pa, PlayerPayloadOwned,
};
use qexed_plugin_api::services::WorldEditRegionSpec;
use qexed_plugin_api::{
    ChatCommandEvent, ConfigReloadEvent, EventContext, LogLevel, NpcInteractEvent,
    NpcMutationsEvent, PlayerAttackEvent, PlayerDeathEvent, PlayerInfo, PlayerJoinEvent,
    PlayerLeaveEvent, PlayerTickEvent, PlayerUseItemEvent, ProjectileHitPlayerEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::declare_simple_plugin!("pvp_arena", "PVP Arena", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        // v4 init 语义：清 duel 队列 + 登记编辑区 + 建场
        clear_duel_queue();
        let config = load_config(&sdk);
        register_regions(&sdk, &config);
        if config.enable && config.build_maps {
            build_maps(&sdk, &config);
        }
        ctx.register_command(COMMAND_NAME);
        ctx.log(LogLevel::Info, "pvp arena initialized");

        // —— /pvp 命令（v4 command_execute + commands 导出）——
        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != COMMAND_NAME {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let player = &ev.player;
                let args = ev
                    .argument
                    .split_whitespace()
                    .map(|part| part.to_ascii_lowercase())
                    .collect::<Vec<_>>();
                let actions = match args.first().map(String::as_str) {
                    None | Some("") | Some("menu") => vec![open_menu(&config)],
                    Some("duel") | Some("match") | Some("1v1") => join_duel_queue(&sdk, &config, player),
                    Some("join") if args.get(1).is_some_and(|m| m == "1v1" || m == "duel") => {
                        join_duel_queue(&sdk, &config, player)
                    }
                    Some("ffa") => join_ffa(&sdk, &config, player),
                    Some("leave") | Some("lobby") | Some("quit") => {
                        leave_game(&sdk, &config, player, "已返回 PVP 大厅")
                    }
                    Some("server") | Some("hub") => leave_to_server_lobby(&sdk, &config, player),
                    Some("stats") => show_stats(&sdk, &player.uuid),
                    Some("status") => show_status(&player.uuid),
                    _ => vec![message(
                        "用法: /pvp, /pvp duel, /pvp ffa, /pvp leave, /pvp server, /pvp stats",
                    )],
                };
                dispatch(&sdk, player, actions);
            })
        });

        // —— 配置热重载（宿主接线后自动生效）——
        ctx.register::<ConfigReloadEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("pvp_arena/config.toml")
                    || path.ends_with("config.toml") && path.contains("pvp_arena"))
                {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_regions(&sdk, &config);
                if config.enable && config.build_maps {
                    build_maps(&sdk, &config);
                }
                api.log(LogLevel::Info, "pvp_arena config reloaded");
            })
        });

        // —— 进服：清状态 + 补档案 + 回大厅 ——
        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_online(&ev.player);
                clear_player_state(&sdk, &ev.player.uuid);
                ensure_stats(&sdk, &ev.player.uuid, &ev.player.username);
                if config.enable {
                    // v4 经 pending 表延迟下发；v6 即时下发
                    dispatch(&sdk, &ev.player, lobby_actions(&config, "已回到 PVP 大厅"));
                }
            })
        });

        // —— 退服：离队 + 对手判胜 ——
        ctx.register::<PlayerLeaveEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                unregister_online(&ev.player.uuid);
                remove_from_queue(&ev.player.uuid);
                finish_session_on_leave(&sdk, &config, &ev.player.uuid, &ev.player.username);
                clear_player_state(&sdk, &ev.player.uuid);
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

        // —— NPC 交互 → 打开菜单 ——
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

        // —— 每 tick：BossBar 状态刷新（宿主接线后自动生效）——
        ctx.register::<PlayerTickEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                if let Some(session) = get_session(&sdk, &ev.player.uuid) {
                    let color = if session.mode == GameModeKind::Duel { "red" } else { "green" };
                    sdk.perform(
                        &ev.player,
                        Pa::BossBar {
                            id: BOSS_BAR_ID.to_string(),
                            title: status_bar_title(&session),
                            progress: 1.0,
                            color: color.to_string(),
                            overlay: "progress".to_string(),
                        },
                    );
                }
            })
        });

        // —— 攻击判局（宿主接线后自动生效）——
        // v6 事件无 target_player/伤害改写/actions 响应，完整 v4 语义（按 uuid 判局 +
        // cancel 原生伤害 + DamagePlayer 自定义伤害/击退）待宿主补齐载荷后恢复。
        ctx.register::<PlayerAttackEvent>(move |_api, _ev, _| {
            Box::pin(async move {
                // v4：无 session 的攻击者不得伤害局内玩家（按 target_player 判）；
                // v6 拿不到目标玩家，暂不干预。
            })
        });

        // —— 手持物品使用：返回物品 / 射箭 ——
        ctx.register::<PlayerUseItemEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                let player = &ev.player;
                // v4 语义 action == "use_item"；v6 首批恒为 "use"
                if ev.action == "use" || ev.action == "use_item" {
                    if is_lobby_return_item(&config, &ev.item) {
                        leave_game(&sdk, &config, player, "已返回 PVP 大厅");
                        return;
                    }
                    if is_server_lobby_return_item(&config, &ev.item) {
                        leave_to_server_lobby(&sdk, &config, player);
                        return;
                    }
                }
                if ev.action != "release_use_item" {
                    return;
                }
                let Some(mut session) = get_session(&sdk, &player.uuid) else {
                    return;
                };
                if !session.alive || !is_pvp_bow(&config, &ev.item) {
                    return;
                }
                let now = now_millis();
                if now.saturating_sub(session.last_shot_ms) < config.combat.bow_cooldown_ms {
                    sdk.perform(player, message("PVP 弓冷却中"));
                    return;
                }
                session.last_shot_ms = now;
                save_session(&sdk, &player.uuid, &session);
                sdk.perform(player, pvp_arrow_projectile(&config, &ev));
            })
        });

        // —— 弹射物命中（宿主弹射物运行时接线后自动生效）——
        ctx.register::<ProjectileHitPlayerEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                if ev.configured_event != PVP_ARROW_EVENT || ev.projectile_kind != "arrow" {
                    return;
                }
                if !ev.tag.trim().ends_with(config.kit.bow_name.trim()) {
                    return;
                }
                let Some(shooter_session) = get_session(&sdk, &ev.shooter.uuid) else {
                    return;
                };
                let Some(target_session) = get_session(&sdk, &ev.target.uuid) else {
                    return;
                };
                if !can_damage(&shooter_session, &target_session, &ev.shooter.uuid, &ev.target.uuid) {
                    return;
                }
                // 命中走 DamagePlayer（v4 同款；宿主当前降级为 HurtAnimation 视觉）
                sdk.perform(
                    &ev.target.clone(),
                    Pa::DamagePlayer {
                        uuid: ev.target.uuid.clone(),
                        username: String::new(),
                        amount: config.combat.arrow_damage.max(0.0),
                        kind: "projectile".to_string(),
                        source_entity_id: ev.shooter.entity_id,
                        source_x: ev.position.x,
                        source_y: ev.position.y,
                        source_z: ev.position.z,
                        knockback: config.combat.arrow_knockback.max(0.0),
                    },
                );
            })
        });

        // —— 死亡判负（宿主接线后自动生效）——
        ctx.register::<PlayerDeathEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                let Some(session) = get_session(&sdk, &ev.player.uuid) else {
                    return;
                };
                if session.mode == GameModeKind::Duel {
                    award_duel_win(
                        &sdk,
                        &config,
                        &session.opponent_uuid,
                        &ev.player.uuid,
                        format!("{} 被击败", ev.player.username),
                    );
                    // v4 置 cancel + message 免死回大厅；v6 事件只读（Clone 分发），
                    // 免死回大厅语义由宿主支持后恢复。
                    return;
                }
                add_death(&sdk, &ev.player.uuid, &ev.player.username);
                clear_player_state(&sdk, &ev.player.uuid);
                save_session(
                    &sdk,
                    &ev.player.uuid,
                    &PlayerSession {
                        mode: GameModeKind::Ffa,
                        game_id: "ffa".to_string(),
                        opponent_uuid: String::new(),
                        opponent_name: String::new(),
                        alive: true,
                        last_shot_ms: 0,
                    },
                );
                dispatch(
                    &sdk,
                    &ev.player,
                    ffa_spawn_actions(&config, "你已阵亡，已重新进入 FFA"),
                );
            })
        });
    })
});

const COMMAND_NAME: &str = "pvp";
const NPC_KEY: &str = "pvp_entry";
const NPC_EVENT: &str = "pvp_menu";
const SESSION_PREFIX: &str = "pvp/session/";
const STATS_PREFIX: &str = "pvp/stats/";
const BOSS_BAR_ID: &str = "pvp_status"; // v4 "pvp:status"：v6 存储/动作 id 不允许冒号
const PVP_ARROW_EVENT: &str = "pvp_arrow";
const WORLD_EDIT_BATCH_LIMIT: usize = 4096;

// ============ 动作构造（对应 v4 *_actions / kit / projectile helpers） ============

fn duel_start_actions(config: &Config, spawn_a: bool, opponent: &str) -> Vec<Pa> {
    let spawn = if spawn_a { &config.duel.spawn_a } else { &config.duel.spawn_b };
    let mut actions = vec![
        Pa::SetPlayersVisible { visible: true },
        Pa::Teleport {
            dimension: config.duel.dimension.clone(),
            x: spawn.x,
            y: spawn.y,
            z: spawn.z,
            yaw: Some(spawn.yaw),
            pitch: Some(spawn.pitch),
        },
        Pa::ResetInventory { restore_menu_items: false },
        message(format!("1v1 开始！对手: {opponent}")),
    ];
    actions.extend(kit_actions(config));
    actions
}

fn ffa_spawn_actions(config: &Config, text: impl Into<String>) -> Vec<Pa> {
    let mut actions = vec![
        Pa::SetPlayersVisible { visible: true },
        Pa::Teleport {
            dimension: config.ffa.dimension.clone(),
            x: config.ffa.spawn.x,
            y: config.ffa.spawn.y,
            z: config.ffa.spawn.z,
            yaw: Some(config.ffa.spawn.yaw),
            pitch: Some(config.ffa.spawn.pitch),
        },
        Pa::ResetInventory { restore_menu_items: false },
        message(text.into()),
    ];
    actions.extend(kit_actions(config));
    actions
}

fn lobby_actions(config: &Config, text: impl Into<String>) -> Vec<Pa> {
    vec![
        Pa::SetPlayersVisible { visible: true },
        Pa::Teleport {
            dimension: config.lobby.dimension.clone(),
            x: config.lobby.spawn_x,
            y: config.lobby.spawn_y,
            z: config.lobby.spawn_z,
            yaw: Some(config.lobby.spawn_yaw),
            pitch: Some(config.lobby.spawn_pitch),
        },
        Pa::ResetInventory { restore_menu_items: true },
        Pa::RemoveBossBar { id: BOSS_BAR_ID.to_string() },
        message(text.into()),
    ]
}

fn kit_actions(config: &Config) -> Vec<Pa> {
    let mut actions = vec![
        give(&config.kit.sword, 1, &config.kit.sword_name, None),
        give(&config.kit.bow, 1, &config.kit.bow_name, None),
        give("minecraft:arrow", config.kit.arrows.clamp(1, 64), "", None),
        give(&config.kit.food, config.kit.food_count.clamp(1, 64), "", None),
    ];
    if config.kit.golden_apples > 0 {
        actions.push(give(
            &config.kit.golden_apple_item,
            config.kit.golden_apples.clamp(1, 64),
            "",
            None,
        ));
    }
    actions.push(give(
        &config.kit.lobby_item,
        1,
        &config.kit.lobby_item_name,
        Some(vec!["右键返回 PVP 大厅".to_string()]),
    ));
    actions.push(give(
        &config.kit.server_lobby_item,
        1,
        &config.kit.server_lobby_item_name,
        Some(vec!["右键返回浅屿闲游大厅".to_string()]),
    ));
    actions
}

fn give(item: &str, count: i32, name: &str, lore: Option<Vec<String>>) -> Pa {
    Pa::GiveItem {
        item: item.to_string(),
        count,
        name: name.to_string(),
        lore: lore.unwrap_or_default(),
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
    }
}

fn is_lobby_return_item(config: &Config, item: &ItemStackPayload) -> bool {
    item.item_name == config.kit.lobby_item
        && item.display_name.trim().ends_with(config.kit.lobby_item_name.trim())
}

fn is_server_lobby_return_item(config: &Config, item: &ItemStackPayload) -> bool {
    item.item_name == config.kit.server_lobby_item
        && item.display_name.trim().ends_with(config.kit.server_lobby_item_name.trim())
}

fn is_pvp_bow(config: &Config, item: &ItemStackPayload) -> bool {
    item.item_name == config.kit.bow
        && item.display_name.trim().ends_with(config.kit.bow_name.trim())
}

fn pvp_arrow_projectile(_config: &Config, ev: &PlayerUseItemEvent) -> Pa {
    let (dir_x, dir_y, dir_z) = look_direction(ev.yaw, ev.pitch);
    let speed = 1.9;
    Pa::SpawnProjectile {
        kind: "arrow".to_string(),
        configured_event: PVP_ARROW_EVENT.to_string(),
        tag: ev.item.display_name.clone(),
        dimension: ev.dimension.clone(),
        x: ev.position.x + dir_x * 0.6,
        y: ev.position.y + 1.55 + dir_y * 0.6,
        z: ev.position.z + dir_z * 0.6,
        velocity_x: dir_x * speed,
        velocity_y: dir_y * speed,
        velocity_z: dir_z * speed,
        source_entity_id: ev.player.entity_id,
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
    (-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos())
}

// ============ NPC / 世界编辑 ============

fn guide_npc(config: &Config) -> NpcUpsert {
    NpcUpsert {
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
    }
}

fn register_regions(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let regions: [(&str, &String, i32, i32, i32, i32); 3] = [
        ("pvp_lobby", &config.lobby.dimension, -24, 24, config.lobby.floor_y, 16),
        ("pvp_duel", &config.duel.dimension, -40, 40, config.duel.floor_y, 24),
        ("pvp_ffa", &config.ffa.dimension, -56, 56, config.ffa.floor_y, 24),
    ];
    for (id, dimension, min, max, floor_y, height) in regions {
        let _ = sdk.world_register_edit_region(&WorldEditRegionSpec {
            id: id.to_string(),
            dimension: dimension.clone(),
            min: (min, floor_y - 4, min),
            max: (max, floor_y + height, max),
            allow_player_break: false,
            allow_player_place: false,
            allow_plugin_write: true,
            runtime_only: true,
        });
    }
}

fn build_maps(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    build_platform(
        sdk,
        &config.lobby.dimension,
        config.lobby.floor_y,
        config.lobby.radius,
        "minecraft:smooth_quartz",
        "minecraft:sea_lantern",
    );
    build_platform(
        sdk,
        &config.duel.dimension,
        config.duel.floor_y,
        config.duel.radius,
        "minecraft:polished_deepslate",
        "minecraft:redstone_lamp",
    );
    build_platform(
        sdk,
        &config.ffa.dimension,
        config.ffa.floor_y,
        config.ffa.radius,
        "minecraft:stone_bricks",
        "minecraft:glowstone",
    );
}

fn build_platform(sdk: &qexed_plugin_sdk::Sdk, dimension: &str, floor_y: i32, radius: i32, floor: &str, marker: &str) {
    let radius = radius.clamp(4, 64);
    let mut blocks: Vec<(String, (i32, i32, i32), String)> = Vec::new();
    for x in -radius..=radius {
        for z in -radius..=radius {
            blocks.push((dimension.to_string(), (x, floor_y, z), floor.to_string()));
            if x.abs() == radius || z.abs() == radius {
                blocks.push((dimension.to_string(), (x, floor_y + 1, z), "minecraft:glass_pane".to_string()));
            }
            if blocks.len() >= WORLD_EDIT_BATCH_LIMIT {
                flush_blocks(sdk, &mut blocks);
            }
        }
    }
    for (x, z) in [(-radius, -radius), (radius, -radius), (-radius, radius), (radius, radius)] {
        blocks.push((dimension.to_string(), (x, floor_y + 1, z), marker.to_string()));
    }
    if !blocks.is_empty() {
        flush_blocks(sdk, &mut blocks);
    }
}

fn flush_blocks(sdk: &qexed_plugin_sdk::Sdk, blocks: &mut Vec<(String, (i32, i32, i32), String)>) {
    let refs: Vec<(&str, (i32, i32, i32), &str)> =
        blocks.iter().map(|(d, p, b)| (d.as_str(), *p, b.as_str())).collect();
    let _ = sdk.world_set_blocks(&refs);
    blocks.clear();
}

// ============ 工具 ============

fn message(text: impl Into<String>) -> Pa {
    Pa::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn player_info(p: &PlayerPayloadOwned) -> PlayerInfo {
    PlayerInfo::new(p.uuid.clone(), p.username.clone(), p.entity_id, p.dimension.clone())
}

/// 在线玩家快照（join/leave 维护）。v4 pending 表可"稍后按 uuid 下发"；v6 动作派发
/// 需要 &PlayerInfo，duel 判胜/开局要把动作发给非当前触发玩家，故维护这张在线表。
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

// ============ 配置（v4 Config struct + DEFAULT_CONFIG 原样移植） ============

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
    #[serde(default = "d_spawn_x")]
    spawn_x: f64,
    #[serde(default = "d_spawn_y")]
    spawn_y: f64,
    #[serde(default = "d_spawn_z")]
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default = "d_floor_y")]
    floor_y: i32,
    #[serde(default = "d_lobby_radius")]
    radius: i32,
    #[serde(default = "d_npc_x")]
    npc_x: f64,
    #[serde(default = "d_npc_y")]
    npc_y: f64,
    #[serde(default = "d_npc_z")]
    npc_z: f64,
    #[serde(default = "d_npc_yaw")]
    npc_yaw: f32,
    #[serde(default)]
    npc_pitch: f32,
    #[serde(default = "d_npc_entity_type")]
    npc_entity_type: String,
    #[serde(default = "d_npc_display_name")]
    npc_display_name: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            dimension: default_lobby_dimension(),
            spawn_x: d_spawn_x(),
            spawn_y: d_spawn_y(),
            spawn_z: d_spawn_z(),
            spawn_yaw: 0.0,
            spawn_pitch: 0.0,
            floor_y: d_floor_y(),
            radius: d_lobby_radius(),
            npc_x: d_npc_x(),
            npc_y: d_npc_y(),
            npc_z: d_npc_z(),
            npc_yaw: d_npc_yaw(),
            npc_pitch: 0.0,
            npc_entity_type: d_npc_entity_type(),
            npc_display_name: d_npc_display_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ArenaConfig {
    #[serde(default = "default_duel_dimension")]
    dimension: String,
    #[serde(default = "d_floor_y")]
    floor_y: i32,
    #[serde(default = "d_arena_radius")]
    radius: i32,
    #[serde(default)]
    spawn: SpawnConfig,
    #[serde(default = "d_duel_spawn_a")]
    spawn_a: SpawnConfig,
    #[serde(default = "d_duel_spawn_b")]
    spawn_b: SpawnConfig,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            dimension: default_duel_dimension(),
            floor_y: d_floor_y(),
            radius: d_arena_radius(),
            spawn: SpawnConfig::default(),
            spawn_a: d_duel_spawn_a(),
            spawn_b: d_duel_spawn_b(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SpawnConfig {
    #[serde(default = "d_spawn_x")]
    x: f64,
    #[serde(default = "d_spawn_y")]
    y: f64,
    #[serde(default = "d_spawn_z")]
    z: f64,
    #[serde(default)]
    yaw: f32,
    #[serde(default)]
    pitch: f32,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self { x: d_spawn_x(), y: d_spawn_y(), z: d_spawn_z(), yaw: 0.0, pitch: 0.0 }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CombatConfig {
    // v6 PlayerAttackEvent 暂无伤害改写能力，近战字段保留兼容配置、待宿主补齐后启用
    #[allow(dead_code)]
    #[serde(default = "d_sword_damage")]
    sword_damage: f32,
    #[allow(dead_code)]
    #[serde(default = "d_axe_damage")]
    axe_damage: f32,
    #[allow(dead_code)]
    #[serde(default = "d_fist_damage")]
    fist_damage: f32,
    #[serde(default = "d_arrow_damage")]
    arrow_damage: f32,
    #[allow(dead_code)]
    #[serde(default = "d_knockback")]
    knockback: f32,
    #[serde(default = "d_arrow_knockback")]
    arrow_knockback: f32,
    #[serde(default = "d_bow_cooldown_ms")]
    bow_cooldown_ms: i64,
}

impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            sword_damage: d_sword_damage(),
            axe_damage: d_axe_damage(),
            fist_damage: d_fist_damage(),
            arrow_damage: d_arrow_damage(),
            knockback: d_knockback(),
            arrow_knockback: d_arrow_knockback(),
            bow_cooldown_ms: d_bow_cooldown_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct KitConfig {
    #[serde(default = "d_sword")]
    sword: String,
    #[serde(default = "d_sword_name")]
    sword_name: String,
    #[serde(default = "d_bow")]
    bow: String,
    #[serde(default = "d_bow_name")]
    bow_name: String,
    #[serde(default = "d_arrows")]
    arrows: i32,
    #[serde(default = "d_food")]
    food: String,
    #[serde(default = "d_food_count")]
    food_count: i32,
    #[serde(default = "d_golden_apple_item")]
    golden_apple_item: String,
    #[serde(default = "d_golden_apples")]
    golden_apples: i32,
    #[serde(default = "d_lobby_item")]
    lobby_item: String,
    #[serde(default = "d_lobby_item_name")]
    lobby_item_name: String,
    #[serde(default = "d_server_lobby_item")]
    server_lobby_item: String,
    #[serde(default = "d_server_lobby_item_name")]
    server_lobby_item_name: String,
}

impl Default for KitConfig {
    fn default() -> Self {
        Self {
            sword: d_sword(),
            sword_name: d_sword_name(),
            bow: d_bow(),
            bow_name: d_bow_name(),
            arrows: d_arrows(),
            food: d_food(),
            food_count: d_food_count(),
            golden_apple_item: d_golden_apple_item(),
            golden_apples: d_golden_apples(),
            lobby_item: d_lobby_item(),
            lobby_item_name: d_lobby_item_name(),
            server_lobby_item: d_server_lobby_item(),
            server_lobby_item_name: d_server_lobby_item_name(),
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

fn d_spawn_x() -> f64 {
    0.5
}

fn d_spawn_y() -> f64 {
    80.0
}

fn d_spawn_z() -> f64 {
    0.5
}

fn d_floor_y() -> i32 {
    80
}

fn d_lobby_radius() -> i32 {
    12
}

fn d_arena_radius() -> i32 {
    24
}

fn d_npc_x() -> f64 {
    2.5
}

fn d_npc_y() -> f64 {
    81.0
}

fn d_npc_z() -> f64 {
    2.5
}

fn d_npc_yaw() -> f32 {
    180.0
}

fn d_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn d_npc_display_name() -> String {
    "{\"text\":\"PVP 竞技\",\"color\":\"red\"}".to_string()
}

fn d_duel_spawn_a() -> SpawnConfig {
    SpawnConfig { x: -8.5, y: 81.0, z: 0.5, yaw: -90.0, pitch: 0.0 }
}

fn d_duel_spawn_b() -> SpawnConfig {
    SpawnConfig { x: 8.5, y: 81.0, z: 0.5, yaw: 90.0, pitch: 0.0 }
}

fn d_sword_damage() -> f32 {
    6.0
}

fn d_axe_damage() -> f32 {
    7.0
}

fn d_fist_damage() -> f32 {
    1.0
}

fn d_arrow_damage() -> f32 {
    3.0
}

fn d_knockback() -> f32 {
    0.42
}

fn d_arrow_knockback() -> f32 {
    0.55
}

fn d_bow_cooldown_ms() -> i64 {
    900
}

fn d_sword() -> String {
    "minecraft:iron_sword".to_string()
}

fn d_sword_name() -> String {
    "PVP 铁剑".to_string()
}

fn d_bow() -> String {
    "minecraft:bow".to_string()
}

fn d_bow_name() -> String {
    "PVP 弓".to_string()
}

fn d_arrows() -> i32 {
    32
}

fn d_food() -> String {
    "minecraft:cooked_beef".to_string()
}

fn d_food_count() -> i32 {
    16
}

fn d_golden_apple_item() -> String {
    "minecraft:enchanted_golden_apple".to_string()
}

fn d_golden_apples() -> i32 {
    2
}

fn d_lobby_item() -> String {
    "minecraft:barrier".to_string()
}

fn d_lobby_item_name() -> String {
    "返回 PVP 大厅".to_string()
}

fn d_server_lobby_item() -> String {
    "minecraft:ender_pearl".to_string()
}

fn d_server_lobby_item_name() -> String {
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

// ============ 配置加载（tiankeng 模式：read_config → DocumentMut → from_document） ============

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
        .expect("default pvp_arena config is valid")
}

// ============ 运行时状态（duel 队列；v4 存储版改内存，语义一致） ============

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueEntry {
    uuid: String,
    username: String,
    joined_at_ms: i64,
}

#[derive(Debug, Default)]
struct DuelQueue {
    players: Vec<QueueEntry>,
}

/// v4 用存储键 "pvp/queue/duel" 持久化队列；v6 原生插件进程内 Mutex 即可（重启即清，
/// 与 v4 init 时 storage_delete(DUEL_QUEUE_KEY) 的清理时机一致）。
fn duel_queue() -> &'static Mutex<DuelQueue> {
    static QUEUE: OnceLock<Mutex<DuelQueue>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(DuelQueue::default()))
}

fn clear_duel_queue() {
    duel_queue().lock().unwrap().players.clear();
}

fn remove_from_queue(player_uuid: &str) {
    duel_queue().lock().unwrap().players.retain(|e| e.uuid != player_uuid);
}

fn pop_opponent(self_uuid: &str) -> Option<QueueEntry> {
    let mut queue = duel_queue().lock().unwrap();
    queue.players.retain(|e| e.uuid != self_uuid);
    queue.players.pop()
}

// ============ 会话与档案（JSON 存储，对应 v4 storage_get_typed/set_typed） ============

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum GameModeKind {
    Duel,
    Ffa,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PlayerStats {
    username: String,
    wins: u32,
    losses: u32,
    kills: u32,
    deaths: u32,
}

fn session_key(player_uuid: &str) -> String {
    format!("{SESSION_PREFIX}{player_uuid}")
}

fn stats_key(player_uuid: &str) -> String {
    format!("{STATS_PREFIX}{player_uuid}")
}

fn get_session(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) -> Option<PlayerSession> {
    sdk.storage_get_json(&session_key(player_uuid))
}

fn save_session(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str, session: &PlayerSession) {
    let _ = sdk.storage_set_json(&session_key(player_uuid), session);
}

fn clear_player_state(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) {
    let _ = sdk.storage_delete(&session_key(player_uuid));
}

fn get_stats(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) -> Option<PlayerStats> {
    sdk.storage_get_json(&stats_key(player_uuid))
}

fn ensure_stats(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str, username: &str) {
    let mut stats = get_stats(sdk, player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    let _ = sdk.storage_set_json(&stats_key(player_uuid), &stats);
}

fn add_win(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str, username: &str) {
    let mut stats = get_stats(sdk, player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    stats.wins += 1;
    let _ = sdk.storage_set_json(&stats_key(player_uuid), &stats);
}

fn add_loss(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str, username: &str) {
    let mut stats = get_stats(sdk, player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    stats.losses += 1;
    let _ = sdk.storage_set_json(&stats_key(player_uuid), &stats);
}

fn add_death(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str, username: &str) {
    let mut stats = get_stats(sdk, player_uuid).unwrap_or_default();
    stats.username = username.to_string();
    stats.deaths += 1;
    let _ = sdk.storage_set_json(&stats_key(player_uuid), &stats);
}

// ============ 玩法流程 ============

fn open_menu(config: &Config) -> Pa {
    Pa::OpenMenu { menu: config.menu_id.clone() }
}

fn join_duel_queue(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player: &PlayerInfo) -> Vec<Pa> {
    if get_session(sdk, &player.uuid).is_some() {
        return vec![message("你已经在对局中，请先 /pvp leave")];
    }
    if let Some(opponent) = pop_opponent(&player.uuid) {
        start_duel(sdk, config, player, opponent)
    } else {
        duel_queue().lock().unwrap().players.push(QueueEntry {
            uuid: player.uuid.clone(),
            username: player.username.clone(),
            joined_at_ms: now_millis(),
        });
        vec![
            message("已进入 1v1 匹配队列"),
            Pa::BossBar {
                id: BOSS_BAR_ID.to_string(),
                title: "PVP | 1v1 匹配中".to_string(),
                progress: 1.0,
                color: "yellow".to_string(),
                overlay: "progress".to_string(),
            },
        ]
    }
}

fn start_duel(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    player: &PlayerInfo,
    opponent: QueueEntry,
) -> Vec<Pa> {
    let game_id = format!("duel_{}", now_millis());
    save_session(
        sdk,
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
        sdk,
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

    // v4：对手动作入 pending 表，player_tick 下发；v6：查在线即发，离线则下 tick join 再清
    let opponent_info = online_player(&opponent.uuid);
    let opponent_actions = duel_start_actions(config, true, &player.username);
    match opponent_info {
        Some(info) => dispatch(sdk, &info, opponent_actions),
        None => clear_player_state(sdk, &opponent.uuid),
    }
    duel_start_actions(config, false, &opponent.username)
}

fn join_ffa(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player: &PlayerInfo) -> Vec<Pa> {
    remove_from_queue(&player.uuid);
    save_session(
        sdk,
        &player.uuid,
        &PlayerSession {
            mode: GameModeKind::Ffa,
            game_id: "ffa".to_string(),
            opponent_uuid: String::new(),
            opponent_name: String::new(),
            alive: true,
            last_shot_ms: 0,
        },
    );
    ffa_spawn_actions(config, "已进入 FFA 竞技场")
}

fn leave_game(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player: &PlayerInfo, text: &str) -> Vec<Pa> {
    remove_from_queue(&player.uuid);
    if let Some(session) = get_session(sdk, &player.uuid) {
        if session.mode == GameModeKind::Duel && !session.opponent_uuid.is_empty() {
            let name = get_stats(sdk, &player.uuid)
                .map(|stats| stats.username)
                .unwrap_or_else(|| "对手".to_string());
            award_duel_win(sdk, config, &session.opponent_uuid, &player.uuid, format!("{name} 退出了对局"));
        }
    }
    clear_player_state(sdk, &player.uuid);
    lobby_actions(config, text)
}

fn leave_to_server_lobby(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player: &PlayerInfo) -> Vec<Pa> {
    let mut actions = leave_game(sdk, config, player, "正在返回服务器大厅");
    actions.push(Pa::ProxyConnect {
        server: config.server_lobby_id.clone(),
        message: String::new(),
    });
    actions
}

fn show_status(player_uuid: &str) -> Vec<Pa> {
    let queue = duel_queue().lock().unwrap();
    if queue.players.iter().any(|e| e.uuid == player_uuid) {
        return vec![message(format!(
            "当前在 1v1 队列中，队列人数: {}",
            queue.players.len()
        ))];
    }
    drop(queue);
    vec![message("当前不在 PVP 对局中")]
}

fn show_stats(sdk: &qexed_plugin_sdk::Sdk, player_uuid: &str) -> Vec<Pa> {
    let stats = get_stats(sdk, player_uuid).unwrap_or_default();
    vec![message(format!(
        "PVP 数据 | 胜: {} 负: {} 击杀: {} 死亡: {}",
        stats.wins, stats.losses, stats.kills, stats.deaths
    ))]
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

fn award_duel_win(
    sdk: &qexed_plugin_sdk::Sdk,
    config: &Config,
    winner_uuid: &str,
    loser_uuid: &str,
    reason: String,
) {
    if winner_uuid.is_empty() {
        clear_player_state(sdk, loser_uuid);
        return;
    }
    let winner_name = get_stats(sdk, winner_uuid)
        .map(|stats| stats.username)
        .unwrap_or_else(|| "胜者".to_string());
    let loser_name = get_stats(sdk, loser_uuid)
        .map(|stats| stats.username)
        .unwrap_or_else(|| "败者".to_string());
    add_win(sdk, winner_uuid, &winner_name);
    add_loss(sdk, loser_uuid, &loser_name);
    clear_player_state(sdk, winner_uuid);
    clear_player_state(sdk, loser_uuid);
    let winner_actions = lobby_actions(config, format!("{reason}，你赢得了 1v1"));
    let loser_actions = lobby_actions(config, format!("你输给了 {winner_name}"));
    if let Some(info) = online_player(winner_uuid) {
        dispatch(sdk, &info, winner_actions);
    }
    if let Some(info) = online_player(loser_uuid) {
        dispatch(sdk, &info, loser_actions);
    }
}

fn finish_session_on_leave(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str, username: &str) {
    if let Some(session) = get_session(sdk, player_uuid) {
        if session.mode == GameModeKind::Duel && !session.opponent_uuid.is_empty() {
            award_duel_win(
                sdk,
                config,
                &session.opponent_uuid,
                player_uuid,
                format!("{username} 离线了"),
            );
        }
    }
}

fn status_bar_title(session: &PlayerSession) -> String {
    match session.mode {
        GameModeKind::Duel => format!("PVP | 1v1 对手: {}", session.opponent_name),
        GameModeKind::Ffa => "PVP | FFA 竞技场".to_string(),
    }
}
