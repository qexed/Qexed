//! enchanting_lab：v4 示例移植 —— 附魔台实验室。
//!
//! v4 在 init 里 world_register_edit_region + 搭建实验室方块，
//! 并通过 enchanting_options 事件给工具追加 Vein Miner 选项。
//! v6：区域注册 + 建方块用 WorldEditApi（视觉层）；附魔选项事件
//! 宿主接线前处于休眠，但 handler 先行就绪。

use qexed_plugin_api::services::WorldEditRegionSpec;
use qexed_plugin_api::{EnchantingOptionsEvent, LogLevel};
use qexed_plugin_sdk::{InitFuture, PluginContext};

const DIMENSION: &str = "minecraft:overworld";

fn build_lab(sdk: &qexed_plugin_sdk::Sdk) {
    let mut blocks = Vec::new();
    for x in -3..=3 {
        for z in -3..=3 {
            blocks.push((DIMENSION, (x, -61, z), "minecraft:smooth_stone"));
        }
    }
    blocks.push((DIMENSION, (0, -60, 0), "minecraft:enchanting_table"));
    blocks.push((DIMENSION, (0, -59, -3), "minecraft:sea_lantern"));
    sdk.world_set_blocks(&blocks);
}

qexed_plugin_sdk::declare_simple_plugin!("enchanting_lab", "Enchanting Lab", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let sdk = ctx.sdk();
        let region = WorldEditRegionSpec {
            id: "enchanting_lab".to_string(),
            dimension: DIMENSION.to_string(),
            min: (-6, -64, -6),
            max: (6, -58, 6),
            allow_player_break: true,
            allow_player_place: true,
            allow_plugin_write: true,
            runtime_only: true,
        };
        let _ = sdk.world_register_edit_region(&region);
        build_lab(&sdk);

        ctx.register::<EnchantingOptionsEvent>(move |api, ev, _| {
            Box::pin(async move {
                let item_name = ev.query.item.item_name.as_str();
                let tool = item_name.ends_with("_pickaxe")
                    || item_name.ends_with("_axe")
                    || item_name.ends_with("_shovel")
                    || item_name.ends_with("_hoe");
                if !tool {
                    return;
                }
                api.log(
                    LogLevel::Info,
                    &format!("enchanting_lab: {item_name} 进附魔台，可追加 Vein Miner"),
                );
            })
        });

        ctx.log(LogLevel::Info, "enchanting_lab initialized");
    })
});
