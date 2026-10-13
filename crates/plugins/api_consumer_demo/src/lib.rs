//! api_consumer_demo：v4 示例移植 —— /apitest 调用 provider 服务并回报。

use qexed_plugin_api::{ChatCommandEvent, EventContext};
use qexed_plugin_sdk::{InitFuture, PluginContext};

qexed_plugin_sdk::declare_simple_plugin!("qexed.demo.consumer", "API Consumer Demo", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        ctx.register_command("apitest");
        ctx.register::<ChatCommandEvent>(move |api, ev, ectx: &EventContext| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                // 只响应自己的命令；其他插件的命令照常放行。
                if ev.command != "apitest" {
                    return;
                }
                // 已接管该命令，阻止宿主继续分发。
                ectx.cancel();
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let storage = sdk
                    .economy_storage("qexed:coin")
                    .unwrap_or_else(|| "unknown".to_string());
                let exists = sdk.plugin_service_exists("qexed.demo.echo");
                let response = sdk
                    .plugin_call("qexed.demo.echo", "echo", b"ok")
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .unwrap_or_else(|| "call_failed".to_string());
                sdk.perform(
                    &ev.player,
                    qexed_plugin_sdk::system_message(format!(
                        "api={response};exists={exists};storage={storage}"
                    )),
                );
            })
        });
    })
});
