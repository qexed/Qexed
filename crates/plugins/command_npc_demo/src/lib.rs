//! command_npc_demo：v4 示例移植 —— Hub NPC + /hub 命令 + 占位符。
//!
//! v6：/hub 命令（cancel 接管）、NpcInteractEvent 转发连接、
//! EntityAiTickEvent 巡逻 AI（Mutex 回写）、PlaceholdersEvent（Mutex 回写）。
//! v4 的 custom_entities / npc_mutations 导出、PlayerItemPickupEvent、
//! ProxyConnectResultEvent 在 v6 宿主尚无触发点（休眠就绪）。

use qexed_plugin_api::payload::{
    EntityAiOperation, NpcInteractPayload, PlaceholderReplacement,
};
use qexed_plugin_api::services::RuntimeEntitySpec;
use qexed_plugin_api::{
    ChatCommandEvent, EntityAiTickEvent, EventContext, LogLevel, NpcInteractEvent,
    PlaceholdersEvent, PlayerItemPickupEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};

const NPC_KEY: &str = "plugin_hub_npc";
const DEFAULT_TARGET_SERVER: &str = "survival_1";
const DIAMOND_ITEM: &str = "minecraft:diamond";

qexed_plugin_sdk::declare_simple_plugin!("command_npc_demo", "Command NPC Demo", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        ctx.register_command("hub");

        // 初始化：货币 + 配置
        {
            let sdk = ctx.sdk();
            let _ = sdk.economy_register_currency("qexed:coin", "Coin", "Q", 2);
            let _ = sdk.config_load_or_create("target_server = \"survival_1\"\n");
        }
        ctx.log(LogLevel::Info, "command_npc_demo initialized");

        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != "hub" {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                if ev.argument.eq_ignore_ascii_case("transfer") {
                    let target = target_server(&sdk);
                    sdk.perform_all(&[
                        (&ev.player, qexed_plugin_sdk::system_message(format!("Connecting to {target}..."))),
                        (&ev.player, proxy_connect(target)),
                    ]);
                    return;
                }
                sdk.perform_all(&[
                    (&ev.player, qexed_plugin_sdk::system_message("Teleporting to hub marker...".to_string())),
                    (&ev.player, qexed_plugin_sdk::teleport(String::new(), 0.5, -53.0, 0.5, Some(180.0), Some(0.0))),
                ]);
            })
        });

        // 巡逻卫兵 AI（宿主 AI tick 接线后生效）
        ctx.register::<EntityAiTickEvent>(|_api, ev, _| {
            Box::pin(async move {
                let e = &ev.query.entity;
                if e.custom_type != "demo:patrol_guard" && e.ai != "plugin:demo_patrol" {
                    return;
                }
                let min_x = e.ai_params.get("patrol_min_x").and_then(serde_json::Value::as_f64).unwrap_or(4.0);
                let max_x = e.ai_params.get("patrol_max_x").and_then(serde_json::Value::as_f64).unwrap_or(12.0);
                let speed: f64 = e.ai_params.get("speed").and_then(serde_json::Value::as_f64).unwrap_or(0.05).clamp(0.0, 0.5);
                let direction = if e.position.x > max_x {
                    -1.0
                } else if e.position.x < min_x {
                    1.0
                } else if e.entity_id % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
                ev.response.lock().unwrap().operations = vec![EntityAiOperation::MoveDelta {
                    x: speed * direction,
                    y: 0.0,
                    z: 0.0,
                    yaw: Some(if direction > 0.0 { -90.0 } else { 90.0 }),
                    pitch: Some(0.0),
                }];
            })
        });

        // Hub NPC：右键连服
        ctx.register::<NpcInteractEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let payload: &NpcInteractPayload = &ev.payload;
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                api.log(
                    LogLevel::Info,
                    &format!(
                        "npc_interact: player={}, key={}, action={}",
                        payload.player.username, payload.entity.key, payload.action
                    ),
                );
                if payload.entity.key != NPC_KEY {
                    return;
                }
                let player = qexed_plugin_api::PlayerInfo {
                    uuid: payload.player.uuid.clone(),
                    username: payload.player.username.clone(),
                    entity_id: payload.player.entity_id,
                    dimension: payload.player.dimension.clone(),
                };
                let target = target_server(&sdk);
                sdk.perform_all(&[
                    (&player, qexed_plugin_sdk::system_message("Hub NPC clicked. Connecting...".to_string())),
                    (&player, proxy_connect(target)),
                ]);
            })
        });

        // 占位符 %demo_language% / %demo_target_server%（宿主接线后生效）
        ctx.register::<PlaceholdersEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let language = ev
                    .query
                    .player
                    .as_ref()
                    .map(|p| p.language.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                ev.response.lock().unwrap().replacements = vec![
                    PlaceholderReplacement { key: "demo_language".to_string(), value: language },
                    PlaceholderReplacement { key: "demo_target_server".to_string(), value: target_server(&sdk) },
                ];
            })
        });

        // 钻石拾取拦截（宿主拾取管线接线后生效）
        ctx.register::<PlayerItemPickupEvent>(move |api, ev, ectx: &qexed_plugin_api::EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let is_diamond = Some(889) == ev.item_id || ev.item_name == DIAMOND_ITEM || ev.item_name == "minecraft:diamond_ore";
                if !is_diamond {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let key = format!("pickup/{}", ev.player.uuid);
                let mut stats: PickupStats = sdk
                    .storage_get_json(&key)
                    .unwrap_or_default();
                stats.blocked_pickups = stats.blocked_pickups.saturating_add(1);
                stats.blocked_items = stats.blocked_items.saturating_add(ev.count.max(0) as u64);
                let _ = sdk.storage_set_json(&key, &stats);
                ectx.cancel(); // 宿主拾取管线接线后即生效（事件字段 cancel 语义）
                sdk.perform(
                    &ev.player,
                    qexed_plugin_sdk::system_message(format!("插件已拦截钻石拾取，本地记录 {} 次。", stats.blocked_pickups)),
                );
            })
        });

        // 运行时实体注册（宿主 custom_entities 收集接线后生效）
        let _ = RuntimeEntitySpec {
            key: "demo:patrol_guard".to_string(),
            dimension: "minecraft:overworld".to_string(),
            entity_type: String::new(),
            x: 8.0,
            y: -53.0,
            z: 2.5,
            yaw: 0.0,
            pitch: 0.0,
            display_name: "{\"text\":\"Patrol Guard\",\"color\":\"gold\"}".to_string(),
            ai: "plugin:demo_patrol".to_string(),
            ai_params_json: r#"{"iq":80,"patrol_min_x":4.0,"patrol_max_x":12.0}"#.to_string(),
            auto_jump: true,
        };
    })
});

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct PickupStats {
    blocked_pickups: u64,
    blocked_items: u64,
}

fn proxy_connect(server: String) -> qexed_plugin_api::payload::PlayerAction {
    qexed_plugin_api::payload::PlayerAction::ProxyConnect { server, message: String::new() }
}

fn target_server(sdk: &qexed_plugin_sdk::Sdk) -> String {
    sdk.read_config()
        .ok()
        .flatten()
        .and_then(|c| string_setting(&c, "target_server"))
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TARGET_SERVER.to_string())
}

fn string_setting(config: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    config.lines().find_map(|line| {
        let line = line.trim();
        let value = line.strip_prefix(&prefix)?.trim();
        let value = value.strip_prefix('"')?.strip_suffix('"')?;
        Some(value.to_string())
    })
}
