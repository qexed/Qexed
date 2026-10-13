//! skyblock：v4 示例移植 —— 首次进服发放空岛起步物资。
//!
//! v4 通过 player_join 返回 actions；v6 在 handler 内直接 apply_player_actions。

use qexed_plugin_api::{ChatCommandEvent, EventContext, LogLevel, PlayerJoinEvent};
use qexed_plugin_sdk::{InitFuture, PluginContext};

const STARTER_KIT: &[(&str, i32)] = &[
    ("minecraft:lava_bucket", 1),
    ("minecraft:ice", 2),
    ("minecraft:bone_meal", 8),
    ("minecraft:oak_sapling", 2),
    ("minecraft:dirt", 3),
    ("minecraft:apple", 5),
];

qexed_plugin_sdk::declare_simple_plugin!("skyblock", "Skyblock", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        ctx.register_command("skyblock");
        ctx.register_command("island");
        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                if ev.command != "skyblock" && ev.command != "island" {
                    return;
                }
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let text = if ev.command == "skyblock" {
                    "§6=== Skyblock ===\n§e/skyblock help §7- 显示帮助\n§e/island §7- 空岛信息".to_string()
                } else {
                    "§6=== 空岛信息 ===\n§7出生岛屿：主世界 (0, -60)\n§7资源：熔岩桶 ×1、冰 ×2、骨粉 ×8\n§7刷石机：冰 + 熔岩 = 圆石".to_string()
                };
                sdk.perform(&ev.player, qexed_plugin_sdk::system_message(text));
            })
        });
        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let key = format!("skyblock/joined/{}", ev.player.uuid);
                if sdk.storage_exists(&key) {
                    api.log(LogLevel::Info, &format!("player {} returned", ev.player.username));
                    return;
                }
                api.log(
                    LogLevel::Info,
                    &format!(
                        "new skyblock player: {} (uuid={}), giving starter kit",
                        ev.player.username, ev.player.uuid
                    ),
                );
                sdk.perform(
                    &ev.player,
                    qexed_plugin_sdk::system_message(format!(
                        "§aWelcome to Skyblock, {}! §eStarter items placed in inventory.",
                        ev.player.username
                    )),
                );
                sdk.perform(
                    &ev.player,
                    qexed_plugin_sdk::system_message(
                        "§7Tip: Place ice to get water, lava + water = cobblestone generator."
                            .to_string(),
                    ),
                );
                for (item, count) in STARTER_KIT {
                    sdk.perform(&ev.player, qexed_plugin_sdk::give_item_with_count(*item, *count));
                }
                sdk.storage_set(&key, b"1");
            })
        });
    })
});
