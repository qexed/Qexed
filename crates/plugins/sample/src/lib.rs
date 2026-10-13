//! 样例动态库插件：演示 declare_plugin! 导出、生命周期与事件订阅。
//!
//! 构建产物 crates/plugins/sample 目标目录下的 qexed_plugin_sample.dll/so
//! 复制进服务端 plugins/ 目录即可被加载。

use async_trait::async_trait;
use qexed_plugin_api::{
    EventContext, EventHandler, EventRegistryExt, HostApi, Plugin, PluginError,
    PluginLoadFinishEvent, PluginMeta, PlayerChatEvent, PlayerJoinEvent, PlayerLeaveEvent,
    declare_plugin,
};

/// 演示订阅器：进出场打日志，聊天里出现 "spam" 直接取消。
struct Greeter {
    meta: PluginMeta,
}

impl Greeter {
    fn new() -> Self {
        Self {
            meta: PluginMeta {
                id: "sample",
                name: "Sample Greeter",
                version: "0.1.0",
                dependencies: &[],
            },
        }
    }
}

impl Default for Greeter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for Greeter {
    fn meta(&self) -> &PluginMeta {
        &self.meta
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn on_enable(&self, api: &dyn HostApi) -> Result<(), PluginError> {
        let me = api.get_plugin_handle("sample").ok_or_else(|| {
            PluginError::new("自身 handle 缺失：宿主未在本插件 enable 前登记")
        })?;
        api.register::<PluginLoadFinishEvent>(me, Box::new(LoadFinishLogger));
        api.register::<PlayerJoinEvent>(me, Box::new(JoinGreeter));
        api.register::<PlayerLeaveEvent>(me, Box::new(ByeLogger));
        api.register::<PlayerChatEvent>(me, Box::new(SpamFilter));
        api.log(qexed_plugin_api::LogLevel::Info, "sample 插件已启用");
        Ok(())
    }
}

struct LoadFinishLogger;

#[async_trait]
impl EventHandler<PluginLoadFinishEvent> for LoadFinishLogger {
    async fn handle(
        &self,
        api: &dyn HostApi,
        _event: &PluginLoadFinishEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            qexed_plugin_api::LogLevel::Info,
            "sample: 全部插件加载完成",
        );
        Ok(())
    }
}

struct JoinGreeter;

#[async_trait]
impl EventHandler<PlayerJoinEvent> for JoinGreeter {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &PlayerJoinEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            qexed_plugin_api::LogLevel::Info,
            &format!("sample: 欢迎 {}", event.player.username),
        );
        Ok(())
    }
}

struct ByeLogger;

#[async_trait]
impl EventHandler<PlayerLeaveEvent> for ByeLogger {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &PlayerLeaveEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            qexed_plugin_api::LogLevel::Info,
            &format!("sample: {} 离开了", event.player.username),
        );
        Ok(())
    }
}

struct SpamFilter;

#[async_trait]
impl EventHandler<PlayerChatEvent> for SpamFilter {
    async fn handle(
        &self,
        _api: &dyn HostApi,
        event: &PlayerChatEvent,
        ctx: &EventContext,
    ) -> Result<(), PluginError> {
        if event.message.contains("spam") {
            ctx.cancel();
        }
        Ok(())
    }
}

declare_plugin!(Greeter);