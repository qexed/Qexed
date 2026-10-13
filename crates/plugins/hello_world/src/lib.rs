//! hello_world：v4 示例移植 —— 初始化日志 + 配置重载/语言切换事件。
//!
//! v4 原版：export `qexed_plugin_init` / `_config_reload` / `_language_change`，
//! 打印 payload 内容。v6 对应 ConfigReloadEvent / LanguageChangeEvent。

use qexed_plugin_api::{ConfigReloadEvent, LanguageChangeEvent, LogLevel};
use qexed_plugin_sdk::{InitFuture, PluginContext};

qexed_plugin_sdk::declare_simple_plugin!("hello_world", "Hello World", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let api = ctx.api();
        api.log(LogLevel::Info, "hello_world initialized");

        ctx.register::<ConfigReloadEvent>(|api, ev, _| {
            Box::pin(async move {
                api.log(
                    LogLevel::Info,
                    &format!("config loaded: {}", ev.payload.path),
                );
            })
        });

        ctx.register::<LanguageChangeEvent>(|api, ev, _| {
            Box::pin(async move {
                api.log(
                    LogLevel::Info,
                    &format!("current language: {}", ev.payload.language),
                );
            })
        });
    })
});
