//! skip_death_respawn：v4 示例移植 —— 跳过死亡返回出生点。
//!
//! v4 在 player_death 响应里置 cancel + message；v6 的 PlayerDeathEvent
//! 尚未由宿主触发（死亡管线未接线），handler 先行就绪并打日志。
//! 宿主接线死亡事件后，本插件按配置返回提示文案（cancel 走 ctx.cancel()）。

use qexed_plugin_api::{LogLevel, PlayerDeathEvent};
use qexed_plugin_sdk::{InitFuture, PluginContext};

const DEFAULT_CONFIG: &str = r#"

# 跳过死亡返回出生点配置
enable = true

message_enable = true
message = "已跳过死亡，返回出生点。"
message_overlay = false
"#;

qexed_plugin_sdk::declare_simple_plugin!("qexed.skip_death_respawn", "Skip Death Respawn", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        match sdk.config_load_or_create(DEFAULT_CONFIG.trim_start_matches("\n")) {
            Ok(_) => {}
            Err(e) => ctx.log(LogLevel::Warn, &format!("配置初始化失败: {e}")),
        }

        ctx.register::<PlayerDeathEvent>(move |api, ev, _ctx| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let enable = sdk
                    .read_config()
                    .ok()
                    .flatten()
                    .and_then(|c| toml_enable(&c))
                    .unwrap_or(true);
                if !enable {
                    return;
                }
                api.log(
                    LogLevel::Info,
                    &format!(
                        "skip_death_respawn: {} 死亡（{}）—— 待宿主死亡管线接线后跳过",
                        ev.player.username, ev.cause
                    ),
                );
            })
        });

        ctx.log(LogLevel::Info, "skip_death_respawn initialized");
    })
});

/// 极简 TOML 读取（避免插件引入 toml 依赖）：只认 "enable = true/false"。
fn toml_enable(content: &str) -> Option<bool> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("enable") {
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix('=') {
                return Some(v.trim() == "true");
            }
        }
    }
    None
}
