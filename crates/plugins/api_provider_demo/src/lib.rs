//! api_provider_demo：v4 示例移植 —— 注册跨插件 echo 服务。
//!
//! v4 通过 manifest services 声明 + api_call 事件；v6 在 enable 时
//! plugin_register_service 注册 PluginServiceProvider。

use std::sync::Arc;

use qexed_plugin_api::services::PluginServiceProvider;
use qexed_plugin_api::LogLevel;
use qexed_plugin_sdk::{InitFuture, PluginContext};

struct EchoService;

impl PluginServiceProvider for EchoService {
    fn call(&self, _caller: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
        if method != "echo" {
            return None;
        }
        let mut response = b"provider:".to_vec();
        response.extend_from_slice(payload);
        Some(response)
    }
}

qexed_plugin_sdk::declare_simple_plugin!("qexed.demo.provider", "API Provider Demo", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let sdk = ctx.sdk();
        let ok = sdk.plugin_register_service("qexed.demo.echo", Arc::new(EchoService));
        ctx.log(
            LogLevel::Info,
            &format!("api_provider_demo initialized (service registered: {ok})"),
        );
    })
});
