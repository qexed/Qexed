//! 插件引导样板：SimplePlugin + PluginContext + 闭包订阅。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use qexed_plugin_api::{
    Event, EventContext, EventHandler, EventRegistryExt, HostApi, LogLevel, Plugin, PluginError,
    PluginHandle, PluginMeta,
};

/// init / handler 闭包返回的装箱 future（可借用宿主引用）。
pub type ChatFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
pub type InitFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// 简单事件处理器：`(api, event, ctx) -> 装箱 future`。
pub type SimpleHandler<E> = Arc<
    dyn for<'a> Fn(&'a dyn HostApi, &'a E, &'a EventContext) -> ChatFuture<'a> + Send + Sync,
>;

/// init 闭包类型：`PluginContext -> 装箱 future`。
pub type InitFn = Arc<dyn for<'a> Fn(PluginContext<'a>) -> InitFuture<'a> + Send + Sync>;

/// 快捷构造 [`PluginMeta`]（无依赖）。
pub fn plugin_meta(id: &'static str, name: &'static str, version: &'static str) -> PluginMeta {
    PluginMeta { id, name, version, dependencies: &[] }
}

/// 把闭包适配成 [`EventHandler`]（SDK 注册路径的内部粘合）。
pub struct ClosureHandler<E: Event> {
    f: SimpleHandler<E>,
}

#[async_trait]
impl<E: Event> EventHandler<E> for ClosureHandler<E> {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &E,
        ctx: &EventContext,
    ) -> Result<(), PluginError> {
        (self.f)(api, event, ctx).await;
        Ok(())
    }
}

/// init 阶段的插件上下文：宿主 API + 自身身份。
pub struct PluginContext<'a> {
    api: &'a dyn HostApi,
    plugin_id: &'static str,
    handle: PluginHandle,
}

impl<'a> PluginContext<'a> {
    /// 由宿主在 on_enable 时构造（handle 已确定）。
    pub fn new(api: &'a dyn HostApi, plugin_id: &'static str, handle: PluginHandle) -> Self {
        Self { api, plugin_id, handle }
    }

    /// 自行按 id 查 handle 的便利构造。
    pub fn discover(api: &'a dyn HostApi, plugin_id: &'static str) -> Result<Self, PluginError> {
        let handle = api
            .get_plugin_handle(plugin_id)
            .ok_or_else(|| PluginError::new("自身 handle 缺失：宿主未在本插件 enable 前登记"))?;
        Ok(Self { api, plugin_id, handle })
    }

    /// 宿主 API。
    pub fn api(&self) -> &'a dyn HostApi {
        self.api
    }

    /// 自身插件 id（manifest id）。
    pub fn id(&self) -> &'static str {
        self.plugin_id
    }

    /// 自身句柄。
    pub fn handle(&self) -> PluginHandle {
        self.handle
    }

    /// 闭包式事件订阅。
    pub fn register<E: Event>(
        &self,
        f: impl for<'x> Fn(&'x dyn HostApi, &'x E, &'x EventContext) -> ChatFuture<'x> + Send + Sync + 'static,
    ) {
        self.api
            .register::<E>(self.handle, Box::new(ClosureHandler { f: Arc::new(f) }));
    }

    /// 声明插件顶层命令（`/command` 触发 [`qexed_plugin_api::ChatCommandEvent`]）。
    pub fn register_command(&self, command: &str) {
        self.api.register_command(self.plugin_id, command);
    }

    /// 便捷服务包装（存储/经济/世界编辑/实体/动作…）。
    pub fn sdk(&self) -> crate::api::Sdk<'a> {
        crate::api::Sdk::new(self.api, self.plugin_id)
    }

    /// 日志快捷方式。
    pub fn log(&self, level: LogLevel, message: impl AsRef<str>) {
        self.api.log(level, message.as_ref());
    }
}

/// 简单插件骨架：meta 样板 + init 闭包。
pub struct SimplePlugin {
    meta: PluginMeta,
    init: InitFn,
}

impl SimplePlugin {
    pub fn new(meta: PluginMeta, init: impl for<'x> Fn(PluginContext<'x>) -> InitFuture<'x> + Send + Sync + 'static) -> Self {
        Self { meta, init: Arc::new(init) }
    }

    /// meta 引用（宏委托用）。
    pub fn meta(&self) -> &PluginMeta {
        &self.meta
    }

    /// on_enable 主体（宏委托用）。
    pub async fn run_enable(&self, api: &dyn HostApi) -> Result<(), PluginError> {
        let ctx = PluginContext::discover(api, self.meta.id)?;
        (self.init)(ctx).await;
        Ok(())
    }
}

#[async_trait]
impl Plugin for SimplePlugin {
    fn meta(&self) -> &PluginMeta {
        &self.meta
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn on_enable(&self, api: &dyn HostApi) -> Result<(), PluginError> {
        self.run_enable(api).await
    }
}

/// 一条宏声明完整插件（结构体 + Default + Plugin + 入口符号）。
///
/// 插件需要持久状态时在 init 闭包外创建 `Arc`/static 捕获。
#[macro_export]
macro_rules! declare_simple_plugin {
    ($id:literal, $name:literal, $ver:literal, $init:expr) => {
        struct __QexedSdkPlugin($crate::SimplePlugin);

        impl Default for __QexedSdkPlugin {
            fn default() -> Self {
                Self($crate::SimplePlugin::new(
                    $crate::plugin_meta($id, $name, $ver),
                    $init,
                ))
            }
        }

        #[::async_trait::async_trait]
        impl $crate::Plugin for __QexedSdkPlugin {
            fn meta(&self) -> &$crate::PluginMeta {
                self.0.meta()
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            async fn on_enable(
                &self,
                api: &dyn $crate::HostApi,
            ) -> Result<(), $crate::PluginError> {
                self.0.run_enable(api).await
            }
        }

        $crate::declare_plugin!(__QexedSdkPlugin);
    };
}
