//! player_audit：v4 示例移植 —— 进出服审计日志。

use qexed_plugin_api::{LogLevel, PlayerJoinEvent, PlayerLeaveEvent};
use qexed_plugin_sdk::{InitFuture, PluginContext};

qexed_plugin_sdk::declare_simple_plugin!("player_audit", "Player Audit", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        ctx.register::<PlayerJoinEvent>(|api, ev, _| {
            Box::pin(async move {
                api.log(
                    LogLevel::Info,
                    &format!(
                        "player joined: username={}, uuid={}, entity_id={}",
                        ev.player.username, ev.player.uuid, ev.player.entity_id
                    ),
                );
            })
        });
        ctx.register::<PlayerLeaveEvent>(|api, ev, _| {
            Box::pin(async move {
                api.log(
                    LogLevel::Info,
                    &format!("player left: username={}, uuid={}", ev.player.username, ev.player.uuid),
                );
            })
        });
    })
});
