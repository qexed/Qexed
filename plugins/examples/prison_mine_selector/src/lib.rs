use qexed_plugin_sdk::{
    BlockDropItem, BlockDropPosition, BlockDropQuery, BlockDropResponse, ConfigReloadPayload,
    MiningSpeedQuery, MiningSpeedResponse, NpcInteractPayload, NpcMutationOp, NpcMutationResponse,
    NpcUpsert, PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction,
    PlayerItemPickupQuery, PlayerItemPickupResponse, PlayerPayload, PlayerTickPayload,
    PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse, WorldEditRegion,
    config_load_or_create, config_read_to_string, economy_balance, economy_deposit,
    economy_register_currency, economy_withdraw, lottery_roll, random_block_pool_roll,
    storage_get_typed, storage_set_typed, world_register_edit_region, world_set_block,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

qexed_plugin_sdk::qexed_plugin_memory!();

const CONFIG_PATH: &str = "config.toml";
const NPC_KEY_PREFIX: &str = "prison_mine:";
const SELECT_MINE_EVENT: &str = "select_mine";
const COMMAND_NAME: &str = "prison";
const POINTS_CURRENCY_ID: &str = "qexed:points";
const POINTS_CURRENCY_NAME: &str = "点券";
const POINTS_CURRENCY_SYMBOL: &str = "点";
const DEFAULT_AUTO_REFILL_THRESHOLD_PERCENT: i32 = 30;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    300
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    let config = load_config();
    register_currency(&config);
    initialize_mines(&config, config.refill_on_init);
    qexed_plugin_sdk::log("prison_mine_selector initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if !path.ends_with("prison_mine_selector/config.toml") && !path.ends_with(CONFIG_PATH) {
        return;
    }
    let config = load_config();
    register_currency(&config);
    initialize_mines(&config, config.refill_on_config_reload);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(_payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    let config = load_config();
    if config.refill_on_join {
        initialize_mines(&config, true);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_tick(ptr: i32, len: i32) -> i64 {
    let Some(_payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerTickPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    process_pending_refills(&load_config());
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default())
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_mutations(_ptr: i32, _len: i32) -> i64 {
    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse::default());
    }

    let mut operations = Vec::new();
    for target in &config.mines {
        operations.push(NpcMutationOp::Upsert {
            npc: target.npc_upsert(&legacy_npc_key(&target.id), &target.npc_dimension()),
        });
    }

    qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse { operations })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<NpcInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let Some(mine_id) = target_mine_id_from_key(&payload.entity.key) else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if payload.configured_event != SELECT_MINE_EVENT {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let config = load_config();
    let Some(mine) = config.mine(mine_id).cloned() else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    let selection = MineSelection {
        mine_id: mine.id.clone(),
        label: mine.label.clone(),
        dimension: mine.dimension.clone(),
    };
    let _ = storage_set_typed(&selection_key(&payload.player.uuid), &selection);

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::SystemMessage {
                text: format!("当前矿区: {}", mine.label),
                translate: String::new(),
                with: Vec::new(),
                overlay: false,
            },
            PlayerAction::OpenMenu {
                menu: "prison".to_string(),
            },
        ],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_placeholders(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlaceholderQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse::default());
    };
    let config = load_config();
    let selection = payload.player.as_ref().and_then(|player| {
        storage_get_typed::<MineSelection>(&selection_key(&player.uuid)).or_else(|| {
            config
                .mine_by_dimension(&player.dimension)
                .map(MineSelection::from)
        })
    });

    let (mine_id, label, dimension) = selection
        .map(|selection| (selection.mine_id, selection.label, selection.dimension))
        .unwrap_or_else(|| {
            (
                String::new(),
                config.unselected_label.clone(),
                String::new(),
            )
        });
    let balance = payload
        .player
        .as_ref()
        .and_then(|player| economy_balance(&player.uuid, &config.currency_id))
        .unwrap_or(0);
    let points = payload
        .player
        .as_ref()
        .and_then(|player| economy_balance(&player.uuid, POINTS_CURRENCY_ID))
        .unwrap_or(0);
    let (level, level_name, next_level_price) = payload
        .player
        .as_ref()
        .map(|player| {
            let progress = player_progress(&player.uuid);
            let level = config.effective_level(progress.level);
            let name = config.level_name(level);
            let next_price = config
                .next_level(level)
                .map(|next| {
                    format_money(
                        next.upgrade_price,
                        config.currency_fractional_digits,
                        &config.currency_symbol,
                    )
                })
                .unwrap_or_else(|| "MAX".to_string());
            (level, name, next_price)
        })
        .unwrap_or_else(|| {
            let level = config.first_level();
            (level, config.level_name(level), "MAX".to_string())
        });
    let money = format_money(
        balance,
        config.currency_fractional_digits,
        &config.currency_symbol,
    );

    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: vec![
            PlaceholderReplacement {
                key: "prison_mine".to_string(),
                value: label,
            },
            PlaceholderReplacement {
                key: "prison_mine_id".to_string(),
                value: mine_id,
            },
            PlaceholderReplacement {
                key: "prison_mine_dimension".to_string(),
                value: dimension,
            },
            PlaceholderReplacement {
                key: "prison_money".to_string(),
                value: money,
            },
            PlaceholderReplacement {
                key: "prison_balance".to_string(),
                value: balance.to_string(),
            },
            PlaceholderReplacement {
                key: "prison_level".to_string(),
                value: level.to_string(),
            },
            PlaceholderReplacement {
                key: "prison_level_name".to_string(),
                value: level_name,
            },
            PlaceholderReplacement {
                key: "prison_next_level_price".to_string(),
                value: next_level_price,
            },
            PlaceholderReplacement {
                key: "prison_points".to_string(),
                value: points.to_string(),
            },
            PlaceholderReplacement {
                key: "prison_currency_symbol".to_string(),
                value: config.currency_symbol,
            },
        ],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_item_pickup(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerItemPickupQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    };
    let config = load_config();
    if !config.enable || !config.auto_sell || payload.count <= 0 {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    }

    let Some(unit_price) = config.sell_price(&payload.item_name) else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    };
    if unit_price <= 0 {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    }

    register_currency(&config);
    let amount = unit_price.saturating_mul(payload.count as i64);
    let balance = economy_deposit(&payload.player.uuid, &config.currency_id, amount)
        .or_else(|| economy_balance(&payload.player.uuid, &config.currency_id))
        .unwrap_or(0);
    record_mined_value(&payload.player.uuid, amount);
    let mut actions = Vec::new();
    if config.sell_message_enable {
        actions.push(PlayerAction::SystemMessage {
            text: config
                .sell_message
                .replace("{item}", &payload.item_name)
                .replace("{count}", &payload.count.to_string())
                .replace(
                    "{amount}",
                    &format_money(
                        amount,
                        config.currency_fractional_digits,
                        &config.currency_symbol,
                    ),
                )
                .replace(
                    "{balance}",
                    &format_money(
                        balance,
                        config.currency_fractional_digits,
                        &config.currency_symbol,
                    ),
                ),
            translate: String::new(),
            with: Vec::new(),
            overlay: config.sell_message_overlay,
        });
    }

    qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse {
        cancel: false,
        consume: true,
        actions,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: COMMAND_NAME.to_string(),
        description_key: "监狱风云菜单操作".to_string(),
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
    register_currency(&config);
    let mut parts = payload.argument.split_whitespace();
    let subcommand = parts.next().unwrap_or_default();
    let response = match subcommand {
        "open" => PluginCommandResponse {
            handled: true,
            actions: vec![PlayerAction::OpenMenu {
                menu: "prison".to_string(),
            }],
        },
        "select" => {
            let mine_id = parts.next().unwrap_or_default();
            select_mine_command(&config, &payload.player, mine_id)
        }
        "upgrade" | "levelup" => upgrade_level(&config, &payload.player),
        "lobby" | "spawn" | "leave" => lobby_command(&config),
        "claim_starter" => claim_starter_pickaxe(&payload.player),
        "buy" => {
            let tool = parts.next().unwrap_or_default();
            buy_pickaxe(&config, &payload.player, tool)
        }
        "give_points" => {
            let balance = economy_deposit(&payload.player.uuid, POINTS_CURRENCY_ID, 100_000)
                .unwrap_or_default();
            PluginCommandResponse {
                handled: true,
                actions: vec![message(format!("已获得 100000 点券，当前点券: {balance}"))],
            }
        }
        "lottery" => lottery_command(&payload.player),
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![message("未知监狱菜单操作".to_string())],
        },
    };

    qexed_plugin_sdk::response_ptr_len(&response)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_block_drops(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockDropQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    };

    let config = load_config();
    track_mine_break(&config, &payload);

    let blast_level = plugin_enchantment_level(&payload.plugin_enchantments, "prison:blast");
    let hell_furnace =
        plugin_enchantment_level(&payload.plugin_enchantments, "prison:hell_furnace") > 0;
    if blast_level <= 0 && !hell_furnace {
        return qexed_plugin_sdk::response_ptr_len(&BlockDropResponse::default());
    }

    let items = if hell_furnace {
        smelted_drop(&payload.block_name)
            .map(|item_name| {
                vec![BlockDropItem {
                    item_id: -1,
                    item_name: item_name.to_string(),
                    count: 1,
                }]
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let break_positions = payload
        .player_position
        .as_ref()
        .filter(|_| blast_level > 0)
        .map(|position| blast_positions(payload.position, position.yaw, blast_level))
        .unwrap_or_default();

    qexed_plugin_sdk::response_ptr_len(&BlockDropResponse {
        replace: hell_furnace && !items.is_empty(),
        items,
        break_positions,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_mining_speed(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<MiningSpeedQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&MiningSpeedResponse::default());
    };
    let haste = plugin_enchantment_level(&payload.plugin_enchantments, "prison:haste").clamp(0, 5);
    if haste <= 0 {
        return qexed_plugin_sdk::response_ptr_len(&MiningSpeedResponse::default());
    }
    qexed_plugin_sdk::response_ptr_len(&MiningSpeedResponse {
        speed: None,
        multiplier: Some(1.0 + haste as f32 * 0.25),
        add: None,
    })
}

fn select_mine_command(
    config: &Config,
    player: &qexed_plugin_sdk::PlayerPayloadOwned,
    mine_id: &str,
) -> PluginCommandResponse {
    let Some(mine) = config.mine(mine_id).cloned() else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("矿区不存在".to_string())],
        };
    };
    let level = config.effective_level(player_progress(&player.uuid).level);
    if level < mine.required_level {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "{} 需要等级 {}，你当前等级 {} ({})",
                mine.label,
                mine.required_level,
                level,
                config.level_name(level)
            ))],
        };
    }
    let selection = MineSelection {
        mine_id: mine.id.clone(),
        label: mine.label.clone(),
        dimension: mine.dimension.clone(),
    };
    let _ = storage_set_typed(&selection_key(&player.uuid), &selection);
    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!("已选择 {}", mine.label)),
            PlayerAction::Teleport {
                dimension: mine.dimension,
                x: mine.spawn_x,
                y: mine.spawn_y,
                z: mine.spawn_z,
                yaw: Some(mine.spawn_yaw),
                pitch: Some(mine.spawn_pitch),
            },
        ],
    }
}

fn upgrade_level(
    config: &Config,
    player: &qexed_plugin_sdk::PlayerPayloadOwned,
) -> PluginCommandResponse {
    let mut progress = player_progress(&player.uuid);
    let current_level = config.effective_level(progress.level);
    let Some(next_level) = config.next_level(current_level) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "当前已是最高等级 {} ({})",
                current_level,
                config.level_name(current_level)
            ))],
        };
    };

    let paid = next_level.upgrade_price <= 0
        || economy_withdraw(&player.uuid, &config.currency_id, next_level.upgrade_price).is_some();
    if !paid {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!(
                "金币不足，升级到 {} ({}) 需要 {}",
                next_level.level,
                next_level.name,
                format_money(
                    next_level.upgrade_price,
                    config.currency_fractional_digits,
                    &config.currency_symbol,
                )
            ))],
        };
    }

    progress.level = next_level.level;
    save_player_progress(&player.uuid, &progress);
    PluginCommandResponse {
        handled: true,
        actions: vec![message(format!(
            "升级成功：{} ({})",
            next_level.level, next_level.name
        ))],
    }
}

fn lobby_command(config: &Config) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::ProxyConnect {
            server: config.lobby_server.clone(),
            message: String::new(),
        }],
    }
}

fn claim_starter_pickaxe(player: &qexed_plugin_sdk::PlayerPayloadOwned) -> PluginCommandResponse {
    let mut progress = player_progress(&player.uuid);
    if progress.claimed_starter_pickaxe {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("你已经领取过初始木镐".to_string())],
        };
    }
    progress.claimed_starter_pickaxe = true;
    save_player_progress(&player.uuid, &progress);
    PluginCommandResponse {
        handled: true,
        actions: vec![
            message("已领取初始木镐".to_string()),
            give_pickaxe("minecraft:wooden_pickaxe", "初始木镐", 1, &[], &[]),
        ],
    }
}

fn buy_pickaxe(
    config: &Config,
    player: &qexed_plugin_sdk::PlayerPayloadOwned,
    tool: &str,
) -> PluginCommandResponse {
    let Some((item, name, price)) = (match tool {
        "stone" => Some(("minecraft:stone_pickaxe", "石镐", 500)),
        "iron" => Some(("minecraft:iron_pickaxe", "铁镐", 2_500)),
        "diamond" => Some(("minecraft:diamond_pickaxe", "钻石镐", 15_000)),
        "netherite" => Some(("minecraft:netherite_pickaxe", "下界合金镐", 80_000)),
        _ => None,
    }) else {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message("未知镐子类型".to_string())],
        };
    };
    if economy_withdraw(&player.uuid, &config.currency_id, price).is_none() {
        return PluginCommandResponse {
            handled: true,
            actions: vec![message(format!("金币不足，需要 {price}"))],
        };
    }
    PluginCommandResponse {
        handled: true,
        actions: vec![
            message(format!("已购买 {name}，花费 {price} 金币")),
            give_pickaxe(item, name, 1, &[], &[]),
        ],
    }
}

fn lottery_command(player: &qexed_plugin_sdk::PlayerPayloadOwned) -> PluginCommandResponse {
    match lottery_roll(&[("points", 50), ("pickaxe", 50)]).as_deref() {
        Some("points") => {
            let balance = economy_deposit(&player.uuid, POINTS_CURRENCY_ID, 1_000).unwrap_or(0);
            PluginCommandResponse {
                handled: true,
                actions: vec![message(format!("抽奖获得 1000 点券，当前点券: {balance}"))],
            }
        }
        _ => PluginCommandResponse {
            handled: true,
            actions: vec![
                message("抽奖获得终极下界合金镐".to_string()),
                give_pickaxe(
                    "minecraft:netherite_pickaxe",
                    "爆破地狱下界合金镐",
                    1,
                    &[
                        ("minecraft:fortune", 3),
                        ("minecraft:unbreaking", 3),
                        ("minecraft:mending", 1),
                        ("minecraft:efficiency", 5),
                    ],
                    &[
                        ("prison:blast", 3),
                        ("prison:hell_furnace", 1),
                        ("prison:haste", 3),
                    ],
                ),
            ],
        },
    }
}

fn give_pickaxe(
    item: &str,
    name: &str,
    count: i32,
    enchantments: &[(&str, i32)],
    plugin_enchantments: &[(&str, i32)],
) -> PlayerAction {
    PlayerAction::GiveItem {
        item: item.to_string(),
        count,
        name: name.to_string(),
        lore: plugin_enchantments
            .iter()
            .map(|(id, level)| format!("{} {}", plugin_enchantment_label(id), level))
            .collect(),
        enchantments: enchantments
            .iter()
            .map(|(id, level)| qexed_plugin_sdk::ItemEnchantment {
                id: id.to_string(),
                level: *level,
            })
            .collect(),
        plugin_enchantments: plugin_enchantments
            .iter()
            .map(|(id, level)| qexed_plugin_sdk::PluginEnchantment {
                id: id.to_string(),
                level: *level,
            })
            .collect(),
    }
}

fn message(text: String) -> PlayerAction {
    PlayerAction::SystemMessage {
        text,
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

fn player_progress(player_uuid: &str) -> PlayerProgress {
    let mut progress =
        storage_get_typed::<PlayerProgress>(&progress_key(player_uuid)).unwrap_or_default();
    progress.level = progress.level.max(default_player_level());
    progress
}

fn save_player_progress(player_uuid: &str, progress: &PlayerProgress) {
    let _ = storage_set_typed(&progress_key(player_uuid), progress);
}

fn record_mined_value(player_uuid: &str, amount: i64) {
    let mut progress = player_progress(player_uuid);
    progress.total_mined_value = progress.total_mined_value.saturating_add(amount.max(0));
    save_player_progress(player_uuid, &progress);
}

fn plugin_enchantment_level(enchantments: &[qexed_plugin_sdk::PluginEnchantment], id: &str) -> i32 {
    enchantments
        .iter()
        .find(|enchantment| enchantment.id == id)
        .map(|enchantment| enchantment.level)
        .unwrap_or(0)
}

fn smelted_drop(block_name: &str) -> Option<&'static str> {
    match block_name {
        "minecraft:iron_ore" | "minecraft:deepslate_iron_ore" => Some("minecraft:iron_ingot"),
        "minecraft:gold_ore" | "minecraft:deepslate_gold_ore" => Some("minecraft:gold_ingot"),
        "minecraft:copper_ore" | "minecraft:deepslate_copper_ore" => Some("minecraft:copper_ingot"),
        "minecraft:ancient_debris" => Some("minecraft:netherite_scrap"),
        "minecraft:stone" => Some("minecraft:stone"),
        _ => None,
    }
}

fn blast_positions(origin: BlockDropPosition, yaw: f32, level: i32) -> Vec<BlockDropPosition> {
    let side = level.clamp(1, 5);
    let radius = side / 2;
    let yaw = yaw.rem_euclid(360.0);
    let (forward_x, forward_z, side_x, side_z) = if (45.0..135.0).contains(&yaw) {
        (-1, 0, 0, 1)
    } else if (135.0..225.0).contains(&yaw) {
        (0, -1, 1, 0)
    } else if (225.0..315.0).contains(&yaw) {
        (1, 0, 0, 1)
    } else {
        (0, 1, 1, 0)
    };
    let mut positions = Vec::new();
    for depth in 0..side {
        for lateral in -radius..=radius {
            for vertical in -radius..=radius {
                positions.push(BlockDropPosition {
                    x: origin.x + forward_x * depth + side_x * lateral,
                    y: origin.y + vertical,
                    z: origin.z + forward_z * depth + side_z * lateral,
                });
            }
        }
    }
    positions
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default = "default_enable")]
    enable: bool,
    #[serde(default = "default_enable")]
    refill_on_init: bool,
    #[serde(default = "default_enable")]
    refill_on_config_reload: bool,
    #[serde(default)]
    refill_on_join: bool,
    #[serde(default = "default_enable")]
    auto_refill_enable: bool,
    #[serde(default = "default_auto_refill_threshold_percent")]
    auto_refill_threshold_percent: i32,
    #[serde(default = "default_unselected_label")]
    unselected_label: String,
    #[serde(default = "default_lobby_server")]
    lobby_server: String,
    #[serde(default = "default_enable")]
    auto_sell: bool,
    #[serde(default = "default_currency_id")]
    currency_id: String,
    #[serde(default = "default_currency_name")]
    currency_name: String,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
    #[serde(default)]
    currency_fractional_digits: i32,
    #[serde(default = "default_enable")]
    sell_message_enable: bool,
    #[serde(default = "default_sell_message")]
    sell_message: String,
    #[serde(default = "default_enable")]
    sell_message_overlay: bool,
    #[serde(default = "default_levels")]
    levels: Vec<LevelConfig>,
    #[serde(default)]
    sell_items: Vec<SellItemConfig>,
    #[serde(default)]
    mines: Vec<MineConfig>,
}

impl Config {
    fn mine(&self, id: &str) -> Option<&MineConfig> {
        self.mines.iter().find(|mine| mine.id == id)
    }

    fn mine_by_dimension(&self, dimension: &str) -> Option<&MineConfig> {
        self.mines.iter().find(|mine| mine.dimension == dimension)
    }

    fn sell_price(&self, item_name: &str) -> Option<i64> {
        let item_name = item_name.trim();
        if item_name.is_empty() {
            return None;
        }
        let prices = if self.sell_items.is_empty() {
            default_sell_items()
        } else {
            self.sell_items.clone()
        };
        prices
            .iter()
            .find(|entry| entry.item.eq_ignore_ascii_case(item_name))
            .map(|entry| entry.price)
    }

    fn configured_levels(&self) -> Vec<LevelConfig> {
        if self.levels.is_empty() {
            default_levels()
        } else {
            let mut levels = self.levels.clone();
            levels.sort_by_key(|entry| entry.level);
            levels
        }
    }

    fn first_level(&self) -> i32 {
        self.configured_levels()
            .first()
            .map(|entry| entry.level)
            .unwrap_or(1)
    }

    fn max_level(&self) -> i32 {
        self.configured_levels()
            .last()
            .map(|entry| entry.level)
            .unwrap_or(1)
    }

    fn effective_level(&self, level: i32) -> i32 {
        level.max(self.first_level()).min(self.max_level())
    }

    fn level_name(&self, level: i32) -> String {
        let levels = self.configured_levels();
        let effective = self.effective_level(level);
        levels
            .iter()
            .find(|entry| entry.level == effective)
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| effective.to_string())
    }

    fn next_level(&self, current_level: i32) -> Option<LevelConfig> {
        let current_level = self.effective_level(current_level);
        self.configured_levels()
            .into_iter()
            .find(|entry| entry.level > current_level)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LevelConfig {
    level: i32,
    name: String,
    upgrade_price: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct SellItemConfig {
    item: String,
    price: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct MineConfig {
    id: String,
    label: String,
    #[serde(default)]
    required_level: i32,
    dimension: String,
    spawn_x: f64,
    spawn_y: f64,
    spawn_z: f64,
    #[serde(default)]
    spawn_yaw: f32,
    #[serde(default)]
    spawn_pitch: f32,
    #[serde(default)]
    npc_dimension: String,
    npc_x: f64,
    npc_y: f64,
    npc_z: f64,
    #[serde(default)]
    npc_yaw: f32,
    #[serde(default)]
    npc_pitch: f32,
    #[serde(default = "default_npc_entity_type")]
    npc_entity_type: String,
    #[serde(default)]
    npc_name: String,
    #[serde(default)]
    npc_display_name: String,
    #[serde(default = "default_enable")]
    look_at_players: bool,
    #[serde(default = "default_enable")]
    ore_enable: bool,
    #[serde(default = "default_ore_min_x")]
    ore_min_x: i32,
    #[serde(default = "default_ore_max_x")]
    ore_max_x: i32,
    #[serde(default = "default_ore_min_y")]
    ore_min_y: i32,
    #[serde(default = "default_ore_max_y")]
    ore_max_y: i32,
    #[serde(default = "default_ore_min_z")]
    ore_min_z: i32,
    #[serde(default = "default_ore_max_z")]
    ore_max_z: i32,
    #[serde(default = "default_ore_precompute_count")]
    ore_precompute_count: usize,
    #[serde(default)]
    ore_blocks: Vec<OreBlockConfig>,
}

impl MineConfig {
    fn npc_dimension(&self) -> String {
        if self.npc_dimension.trim().is_empty() {
            self.dimension.clone()
        } else {
            self.npc_dimension.clone()
        }
    }

    fn npc_name(&self) -> String {
        if self.npc_name.trim().is_empty() {
            self.label.clone()
        } else {
            self.npc_name.clone()
        }
    }

    fn npc_display_name(&self) -> String {
        if self.npc_display_name.trim().is_empty() {
            self.label.clone()
        } else {
            self.npc_display_name.clone()
        }
    }

    fn npc_upsert(&self, key: &str, dimension: &str) -> NpcUpsert {
        NpcUpsert {
            key: key.to_string(),
            dimension: dimension.to_string(),
            x: self.npc_x,
            y: self.npc_y,
            z: self.npc_z,
            yaw: self.npc_yaw,
            pitch: self.npc_pitch,
            name: self.npc_name(),
            display_name: self.npc_display_name(),
            entity_type: self.npc_entity_type.clone(),
            skin_textures: String::new(),
            skin_signature: String::new(),
            look_at_players: self.look_at_players,
            main_hand_event: SELECT_MINE_EVENT.to_string(),
            off_hand_event: "ignore".to_string(),
            attack_event: "ignore".to_string(),
        }
    }

    fn ore_bounds(&self) -> MineBounds {
        MineBounds {
            min_x: self.ore_min_x.min(self.ore_max_x),
            max_x: self.ore_min_x.max(self.ore_max_x),
            min_y: self.ore_min_y.min(self.ore_max_y),
            max_y: self.ore_min_y.max(self.ore_max_y),
            min_z: self.ore_min_z.min(self.ore_max_z),
            max_z: self.ore_min_z.max(self.ore_max_z),
        }
    }

    fn contains_ore_position(&self, position: BlockDropPosition) -> bool {
        self.ore_bounds().contains(position)
    }

    fn ore_volume(&self) -> usize {
        self.ore_bounds().volume()
    }

    fn ore_blocks(&self) -> Vec<OreBlockConfig> {
        if self.ore_blocks.is_empty() {
            default_ore_blocks_for_level(self.required_level)
        } else {
            self.ore_blocks.clone()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct OreBlockConfig {
    block: String,
    weight: u64,
}

#[derive(Debug, Clone, Copy)]
struct MineBounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
}

impl MineBounds {
    fn contains(&self, position: BlockDropPosition) -> bool {
        position.x >= self.min_x
            && position.x <= self.max_x
            && position.y >= self.min_y
            && position.y <= self.max_y
            && position.z >= self.min_z
            && position.z <= self.max_z
    }

    fn volume(&self) -> usize {
        let x = (self.max_x - self.min_x + 1).max(0) as usize;
        let y = (self.max_y - self.min_y + 1).max(0) as usize;
        let z = (self.max_z - self.min_z + 1).max(0) as usize;
        x.saturating_mul(y).saturating_mul(z)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MineSelection {
    mine_id: String,
    label: String,
    dimension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerProgress {
    #[serde(default = "default_player_level")]
    level: i32,
    #[serde(default)]
    total_mined_value: i64,
    #[serde(default)]
    claimed_starter_pickaxe: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MineRefillState {
    #[serde(default)]
    mined_positions: BTreeSet<String>,
    #[serde(default)]
    pending_refill: bool,
}

impl Default for PlayerProgress {
    fn default() -> Self {
        Self {
            level: default_player_level(),
            total_mined_value: 0,
            claimed_starter_pickaxe: false,
        }
    }
}

impl From<&MineConfig> for MineSelection {
    fn from(mine: &MineConfig) -> Self {
        Self {
            mine_id: mine.id.clone(),
            label: mine.label.clone(),
            dimension: mine.dimension.clone(),
        }
    }
}

fn initialize_mines(config: &Config, refill: bool) {
    if !config.enable {
        return;
    }
    for mine in &config.mines {
        if mine.ore_enable {
            register_ore_region(mine);
            if refill {
                refill_ore_pit(mine);
                reset_mine_refill_state(mine);
            }
        }
    }
}

fn register_ore_region(mine: &MineConfig) {
    let bounds = mine.ore_bounds();
    let _ = world_register_edit_region(&WorldEditRegion {
        id: &format!("{}_ore_pit", mine.id),
        dimension: &mine.dimension,
        min: (bounds.min_x, bounds.min_y, bounds.min_z),
        max: (bounds.max_x, bounds.max_y, bounds.max_z),
        allow_player_break: true,
        allow_player_place: false,
        allow_plugin_write: true,
        runtime_only: true,
    });
}

fn refill_ore_pit(mine: &MineConfig) {
    let bounds = mine.ore_bounds();
    let ore_blocks = mine.ore_blocks();
    let entries = ore_blocks
        .iter()
        .filter(|entry| entry.weight > 0 && !entry.block.trim().is_empty())
        .map(|entry| (entry.block.as_str(), entry.weight))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }

    let mut written = 0usize;
    let pool_id = format!("{}_ore_blocks", mine.id);
    for y in bounds.min_y..=bounds.max_y {
        for z in bounds.min_z..=bounds.max_z {
            for x in bounds.min_x..=bounds.max_x {
                let Some(block) =
                    random_block_pool_roll(&pool_id, &entries, mine.ore_precompute_count)
                else {
                    continue;
                };
                if world_set_block(&mine.dimension, (x, y, z), &block) {
                    written += 1;
                }
            }
        }
    }
    qexed_plugin_sdk::log(&format!(
        "prison mine refilled: mine={}, dimension={}, blocks={written}",
        mine.id, mine.dimension
    ));
}

fn track_mine_break(config: &Config, payload: &BlockDropQuery) {
    if !config.enable || !config.auto_refill_enable {
        return;
    }
    let Some(player) = payload.player.as_ref() else {
        return;
    };
    let Some(mine) = config.mine_by_dimension(&player.dimension) else {
        return;
    };
    if !mine.ore_enable || !mine.contains_ore_position(payload.position) {
        return;
    }

    let key = mine_refill_state_key(&mine.id);
    let mut state = storage_get_typed::<MineRefillState>(&key).unwrap_or_default();
    if state.pending_refill {
        return;
    }
    state
        .mined_positions
        .insert(block_position_key(payload.position));
    if should_refill_mine(config, mine, state.mined_positions.len()) {
        state.pending_refill = true;
        qexed_plugin_sdk::log(&format!(
            "prison mine queued refill: mine={}, mined={}, total={}, threshold={}%",
            mine.id,
            state.mined_positions.len(),
            mine.ore_volume(),
            config.auto_refill_threshold_percent.clamp(1, 99)
        ));
    }
    let _ = storage_set_typed(&key, &state);
}

fn process_pending_refills(config: &Config) {
    if !config.enable || !config.auto_refill_enable {
        return;
    }
    for mine in config.mines.iter().filter(|mine| mine.ore_enable) {
        let key = mine_refill_state_key(&mine.id);
        let state = storage_get_typed::<MineRefillState>(&key).unwrap_or_default();
        if !state.pending_refill {
            continue;
        }
        refill_ore_pit(mine);
        reset_mine_refill_state(mine);
    }
}

fn should_refill_mine(config: &Config, mine: &MineConfig, mined_count: usize) -> bool {
    let total = mine.ore_volume();
    if total == 0 {
        return false;
    }
    let threshold = config.auto_refill_threshold_percent.clamp(1, 99) as usize;
    let remaining = total.saturating_sub(mined_count.min(total));
    remaining.saturating_mul(100) <= total.saturating_mul(threshold)
}

fn reset_mine_refill_state(mine: &MineConfig) {
    let _ = storage_set_typed(&mine_refill_state_key(&mine.id), &MineRefillState::default());
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<Config>(&content).ok())
        .filter(|config| !config.mines.is_empty())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default prison mine selector config is valid")
}

fn legacy_npc_key(mine_id: &str) -> String {
    format!("{NPC_KEY_PREFIX}{mine_id}")
}

fn target_mine_id_from_key(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(NPC_KEY_PREFIX)?;
    rest.rsplit(':').next().filter(|value| !value.is_empty())
}

fn selection_key(player_uuid: &str) -> String {
    format!("selection/{player_uuid}")
}

fn progress_key(player_uuid: &str) -> String {
    format!("progress/{player_uuid}")
}

fn mine_refill_state_key(mine_id: &str) -> String {
    format!("mine_refill/{mine_id}")
}

fn block_position_key(position: BlockDropPosition) -> String {
    format!("{},{},{}", position.x, position.y, position.z)
}

fn default_enable() -> bool {
    true
}

fn default_player_level() -> i32 {
    1
}

fn default_auto_refill_threshold_percent() -> i32 {
    DEFAULT_AUTO_REFILL_THRESHOLD_PERCENT
}

fn default_levels() -> Vec<LevelConfig> {
    [
        (1, "一级矿工", 0),
        (2, "二级矿工", 1_000_000),
        (3, "三级矿工", 3_000_000),
        (4, "四级矿工", 5_000_000),
        (5, "五级矿工", 7_000_000),
        (6, "六级矿工", 9_000_000),
        (7, "七级矿工", 11_000_000),
        (8, "八级矿工", 13_000_000),
        (9, "九级矿王", 15_000_000),
    ]
    .into_iter()
    .map(|(level, name, upgrade_price)| LevelConfig {
        level,
        name: name.to_string(),
        upgrade_price,
    })
    .collect()
}

fn default_unselected_label() -> String {
    "未选择".to_string()
}

fn default_lobby_server() -> String {
    "lobby_1".to_string()
}

fn default_currency_id() -> String {
    "qexed:coin".to_string()
}

fn default_currency_name() -> String {
    "Coin".to_string()
}

fn default_currency_symbol() -> String {
    "$".to_string()
}

fn default_sell_message() -> String {
    "卖出 {count}x {item}，获得 {amount}，余额 {balance}".to_string()
}

fn default_npc_entity_type() -> String {
    "minecraft:zombie".to_string()
}

fn default_ore_min_x() -> i32 {
    38
}

fn default_ore_max_x() -> i32 {
    51
}

fn default_ore_min_y() -> i32 {
    -49
}

fn default_ore_max_y() -> i32 {
    -30
}

fn default_ore_min_z() -> i32 {
    8
}

fn default_ore_max_z() -> i32 {
    21
}

fn default_ore_precompute_count() -> usize {
    512
}

fn plugin_enchantment_label(id: &str) -> &str {
    match id {
        "prison:blast" => "爆破",
        "prison:hell_furnace" => "地狱熔炉",
        "prison:haste" => "急速",
        _ => id,
    }
}

fn register_currency(config: &Config) {
    let _ = economy_register_currency(
        &config.currency_id,
        &config.currency_name,
        &config.currency_symbol,
        config.currency_fractional_digits,
    );
    let _ = economy_register_currency(
        POINTS_CURRENCY_ID,
        POINTS_CURRENCY_NAME,
        POINTS_CURRENCY_SYMBOL,
        0,
    );
}

fn format_money(amount: i64, fractional_digits: i32, symbol: &str) -> String {
    let digits = fractional_digits.clamp(0, 8) as u32;
    if digits == 0 {
        return format!("{symbol}{amount}");
    }
    let scale = 10_i64.pow(digits);
    let sign = if amount < 0 { "-" } else { "" };
    let absolute = amount.abs();
    format!(
        "{sign}{symbol}{}.{:0width$}",
        absolute / scale,
        absolute % scale,
        width = digits as usize
    )
}

fn default_sell_items() -> Vec<SellItemConfig> {
    [
        ("minecraft:cobblestone", 1),
        ("minecraft:stone", 1),
        ("minecraft:coal", 3),
        ("minecraft:coal_ore", 3),
        ("minecraft:raw_copper", 5),
        ("minecraft:copper_ingot", 5),
        ("minecraft:copper_ore", 5),
        ("minecraft:raw_iron", 8),
        ("minecraft:iron_ingot", 8),
        ("minecraft:iron_ore", 8),
        ("minecraft:raw_gold", 12),
        ("minecraft:gold_ingot", 12),
        ("minecraft:gold_ore", 12),
        ("minecraft:redstone", 7),
        ("minecraft:redstone_ore", 7),
        ("minecraft:lapis_lazuli", 10),
        ("minecraft:lapis_ore", 10),
        ("minecraft:diamond", 50),
        ("minecraft:diamond_ore", 50),
        ("minecraft:emerald", 80),
        ("minecraft:emerald_ore", 80),
        ("minecraft:ancient_debris", 150),
        ("minecraft:netherite_scrap", 150),
    ]
    .into_iter()
    .map(|(item, price)| SellItemConfig {
        item: item.to_string(),
        price,
    })
    .collect()
}

fn default_ore_blocks_for_level(level: i32) -> Vec<OreBlockConfig> {
    let block = match level {
        1 => "minecraft:stone",
        2 => "minecraft:coal_ore",
        3 => "minecraft:copper_ore",
        4 => "minecraft:iron_ore",
        5 => "minecraft:gold_ore",
        6 => "minecraft:redstone_ore",
        7 => "minecraft:lapis_ore",
        8 => "minecraft:diamond_ore",
        _ => "minecraft:ancient_debris",
    };
    vec![OreBlockConfig {
        block: block.to_string(),
        weight: 1,
    }]
}

const DEFAULT_CONFIG: &str = r#"enable = true
refill_on_init = true
refill_on_config_reload = true
refill_on_join = false
auto_refill_enable = true
auto_refill_threshold_percent = 30
unselected_label = "未选择"
lobby_server = "lobby_1"
auto_sell = true
currency_id = "qexed:coin"
currency_name = "Coin"
currency_symbol = "$"
currency_fractional_digits = 0
sell_message_enable = true
sell_message = "卖出 {count}x {item}，获得 {amount}，余额 {balance}"
sell_message_overlay = true

[[levels]]
level = 1
name = "一级矿工"
upgrade_price = 0

[[levels]]
level = 2
name = "二级矿工"
upgrade_price = 1000000

[[levels]]
level = 3
name = "三级矿工"
upgrade_price = 3000000

[[levels]]
level = 4
name = "四级矿工"
upgrade_price = 5000000

[[levels]]
level = 5
name = "五级矿工"
upgrade_price = 7000000

[[levels]]
level = 6
name = "六级矿工"
upgrade_price = 9000000

[[levels]]
level = 7
name = "七级矿工"
upgrade_price = 11000000

[[levels]]
level = 8
name = "八级矿工"
upgrade_price = 13000000

[[levels]]
level = 9
name = "九级矿王"
upgrade_price = 15000000

[[sell_items]]
item = "minecraft:cobblestone"
price = 1
[[sell_items]]
item = "minecraft:stone"
price = 1
[[sell_items]]
item = "minecraft:coal"
price = 3
[[sell_items]]
item = "minecraft:coal_ore"
price = 3
[[sell_items]]
item = "minecraft:raw_copper"
price = 5
[[sell_items]]
item = "minecraft:copper_ingot"
price = 5
[[sell_items]]
item = "minecraft:copper_ore"
price = 5
[[sell_items]]
item = "minecraft:raw_iron"
price = 8
[[sell_items]]
item = "minecraft:iron_ingot"
price = 8
[[sell_items]]
item = "minecraft:iron_ore"
price = 8
[[sell_items]]
item = "minecraft:raw_gold"
price = 12
[[sell_items]]
item = "minecraft:gold_ingot"
price = 12
[[sell_items]]
item = "minecraft:gold_ore"
price = 12
[[sell_items]]
item = "minecraft:redstone"
price = 7
[[sell_items]]
item = "minecraft:redstone_ore"
price = 7
[[sell_items]]
item = "minecraft:lapis_lazuli"
price = 10
[[sell_items]]
item = "minecraft:lapis_ore"
price = 10
[[sell_items]]
item = "minecraft:diamond"
price = 50
[[sell_items]]
item = "minecraft:diamond_ore"
price = 50
[[sell_items]]
item = "minecraft:emerald"
price = 80
[[sell_items]]
item = "minecraft:emerald_ore"
price = 80
[[sell_items]]
item = "minecraft:ancient_debris"
price = 150
[[sell_items]]
item = "minecraft:netherite_scrap"
price = 150

[[mines]]
id = "mine_1"
label = "1级矿区"
required_level = 1
dimension = "qexed:mine_1"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 64.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 1"
npc_display_name = "{\"text\":\"1级矿区\",\"color\":\"gray\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:stone"
weight = 1

[[mines]]
id = "mine_2"
label = "2级矿区"
required_level = 2
dimension = "qexed:mine_2"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 65.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 2"
npc_display_name = "{\"text\":\"2级矿区\",\"color\":\"dark_gray\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:coal_ore"
weight = 1

[[mines]]
id = "mine_3"
label = "3级矿区"
required_level = 3
dimension = "qexed:mine_3"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 66.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 3"
npc_display_name = "{\"text\":\"3级矿区\",\"color\":\"gold\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:copper_ore"
weight = 1

[[mines]]
id = "mine_4"
label = "4级矿区"
required_level = 4
dimension = "qexed:mine_4"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 67.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 4"
npc_display_name = "{\"text\":\"4级矿区\",\"color\":\"white\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:iron_ore"
weight = 1

[[mines]]
id = "mine_5"
label = "5级矿区"
required_level = 5
dimension = "qexed:mine_5"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 68.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 5"
npc_display_name = "{\"text\":\"5级矿区\",\"color\":\"yellow\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:gold_ore"
weight = 1

[[mines]]
id = "mine_6"
label = "6级矿区"
required_level = 6
dimension = "qexed:mine_6"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 69.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 6"
npc_display_name = "{\"text\":\"6级矿区\",\"color\":\"red\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:redstone_ore"
weight = 1

[[mines]]
id = "mine_7"
label = "7级矿区"
required_level = 7
dimension = "qexed:mine_7"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 70.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 7"
npc_display_name = "{\"text\":\"7级矿区\",\"color\":\"blue\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:lapis_ore"
weight = 1

[[mines]]
id = "mine_8"
label = "8级矿区"
required_level = 8
dimension = "qexed:mine_8"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 71.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 8"
npc_display_name = "{\"text\":\"8级矿区\",\"color\":\"aqua\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:diamond_ore"
weight = 1

[[mines]]
id = "mine_9"
label = "9级矿区"
required_level = 9
dimension = "qexed:mine_9"
spawn_x = 67.0
spawn_y = -28.0
spawn_z = 15.0
spawn_yaw = 180.0
spawn_pitch = 0.0
npc_x = 72.5
npc_y = -28.0
npc_z = 15.5
npc_yaw = 180.0
npc_pitch = 0.0
npc_entity_type = "minecraft:zombie"
npc_name = "Mine 9"
npc_display_name = "{\"text\":\"9级矿区\",\"color\":\"dark_purple\"}"
look_at_players = true
ore_enable = true
ore_min_x = 38
ore_max_x = 51
ore_min_y = -49
ore_max_y = -30
ore_min_z = 8
ore_max_z = 21
ore_precompute_count = 512
[[mines.ore_blocks]]
block = "minecraft:ancient_debris"
weight = 1
"#;
