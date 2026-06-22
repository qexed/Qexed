use std::any::Any;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

pub use async_trait::async_trait;

// ---------------------------------------------------------------------------
// PluginMeta — 插件元数据
// ---------------------------------------------------------------------------

/// 插件元数据，由插件作者在实现 `Plugin::meta()` 时返回。
///
/// 所有 `&'static str` 字段都可以用常量或字面量。
#[derive(Clone, Debug)]
pub struct PluginMeta {
    /// 唯一标识，如 `"economy"`、`"shop"`
    pub id: &'static str,
    /// 对外展示名称
    pub name: &'static str,
    /// 语义化版本号字符串
    pub version: &'static str,
    /// 依赖的其他插件 ID 列表（按 id）
    pub dependencies: &'static [&'static str],
}

// ---------------------------------------------------------------------------
// PluginError
// ---------------------------------------------------------------------------

/// 插件操作返回的错误。
#[derive(Debug)]
pub struct PluginError {
    pub message: String,
}

impl PluginError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PluginError: {}", self.message)
    }
}

impl std::error::Error for PluginError {}

// ---------------------------------------------------------------------------
// LogLevel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

// ---------------------------------------------------------------------------
// HostApi — 由互相独立的子 trait 组合
// ---------------------------------------------------------------------------

/// 日志接口。
pub trait LogApi: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
}

/// 配置读写接口。
///
/// 文件路径为 `config/<plugin_id>/config.toml`。
pub trait ConfigApi: Send + Sync {
    fn read_config_toml(&self, plugin_id: &str) -> Result<Option<String>, PluginError>;
    fn write_config_toml(&self, plugin_id: &str, toml_content: &str) -> Result<(), PluginError>;
}

// ---------------------------------------------------------------------------
// PluginHandle
// ---------------------------------------------------------------------------

/// 插件整数句柄，等价于数组下标，O(1) 零开销查找。
///
/// 在 `on_enable` 阶段通过 `api.get_plugin_handle("id")` 获取依赖插件的 handle，
/// 存入自己的结构体，后续热路径直接用 `api.get_plugin(handle)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginHandle(pub usize);

// ---------------------------------------------------------------------------
// PluginRefApi
// ---------------------------------------------------------------------------

/// 跨插件引用接口。
pub trait PluginRefApi: Send + Sync {
    /// 通过句柄获取插件引用 — 热路径，O(1) 数组下标访问。
    fn get_plugin(&self, handle: PluginHandle) -> Option<&dyn Plugin>;

    /// 字符串 ID → 句柄 — 冷路径，仅加载阶段调用一次。
    fn get_plugin_handle(&self, id: &str) -> Option<PluginHandle>;
}

// ---------------------------------------------------------------------------
// RuntimeApi
// ---------------------------------------------------------------------------

/// 异步运行时能力 —— 插件可以 spawn 后台任务并行执行。
pub trait RuntimeApi: Send + Sync {
    /// 提交一个后台任务，不阻塞当前流程。
    ///
    /// 任务在服务端的异步运行时中并发执行。
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>);
}

// ---------------------------------------------------------------------------
// EventApi
// ---------------------------------------------------------------------------

/// 事件标记 trait —— 每个事件类型实现它。
pub trait Event: Send + Sync + 'static {}

/// 单次事件分发的上下文。
///
/// handler 可以通过 `cancel()` 标记事件已取消；触发方通过 `EventEmitResult`
/// 读取最终状态。
pub struct EventContext {
    cancelled: AtomicBool,
}

impl EventContext {
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn set_cancelled(&self, cancelled: bool) {
        self.cancelled.store(cancelled, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 事件分发结果。
#[derive(Debug)]
pub struct EventEmitResult {
    pub cancelled: bool,
    pub errors: Vec<PluginError>,
}

impl EventEmitResult {
    pub fn new(cancelled: bool, errors: Vec<PluginError>) -> Self {
        Self { cancelled, errors }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for EventEmitResult {
    fn default() -> Self {
        Self {
            cancelled: false,
            errors: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// 内置事件
// ---------------------------------------------------------------------------

/// 所有插件加载并启用完成时触发。
///
/// 插件在 `on_enable` 中订阅此事件，在 `init()` 末尾被触发。
#[derive(Debug, Clone)]
pub struct PluginLoadFinishEvent;

impl Event for PluginLoadFinishEvent {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkChunkLoadSource {
    Saved,
    LocalGenerated,
    VanillaGenerated,
    EmptyFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkSyncCause {
    InitialLogin,
    PlayerMove,
    CompletionTick,
    UnloadTick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLoadEvent {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub source: NetworkChunkLoadSource,
}

impl ChunkLoadEvent {
    pub fn new(chunk_x: i32, chunk_z: i32, source: NetworkChunkLoadSource) -> Self {
        Self {
            chunk_x,
            chunk_z,
            source,
        }
    }
}

impl Event for ChunkLoadEvent {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkUnloadEvent {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl ChunkUnloadEvent {
    pub fn new(chunk_x: i32, chunk_z: i32) -> Self {
        Self { chunk_x, chunk_z }
    }
}

impl Event for ChunkUnloadEvent {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkSyncEvent {
    pub cause: ChunkSyncCause,
    pub center_chunk_x: i32,
    pub center_chunk_z: i32,
    pub loaded: Vec<ChunkLoadEvent>,
    pub unloaded: Vec<ChunkUnloadEvent>,
    pub unloading: Vec<ChunkUnloadEvent>,
}

impl ChunkSyncEvent {
    pub fn new(
        cause: ChunkSyncCause,
        center_chunk_x: i32,
        center_chunk_z: i32,
        loaded: Vec<ChunkLoadEvent>,
        unloaded: Vec<ChunkUnloadEvent>,
        unloading: Vec<ChunkUnloadEvent>,
    ) -> Self {
        Self {
            cause,
            center_chunk_x,
            center_chunk_z,
            loaded,
            unloaded,
            unloading,
        }
    }

    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    pub fn unloaded_count(&self) -> usize {
        self.unloaded.len()
    }

    pub fn unloading_count(&self) -> usize {
        self.unloading.len()
    }
}

impl Event for ChunkSyncEvent {}

/// 事件处理器 —— 插件实现此 trait 来订阅特定事件。
#[async_trait]
pub trait EventHandler<E: Event>: Send + Sync {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &E,
        ctx: &EventContext,
    ) -> Result<(), PluginError>;
}

/// 类型擦除的事件处理器闭包。
///
/// 跨 DLL 边界安全——闭包由插件（DLL）创建，fire 时直接调用，无需 `Any` downcast。
///
/// `for<'a>` 让返回的 future 能借用入参 `host` / `raw` 的生命周期。
pub type ErasedHandler = Box<
    dyn for<'a> Fn(
            &'a dyn HostApi,
            &'static str,
            &'a (dyn std::any::Any + Send + Sync),
            &'a EventContext,
        ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'a>>
        + Send
        + Sync,
>;

/// 事件注册接口（dyn 兼容，放入 HostApi）。
///
/// 使用 `type_name` 字符串代替 `TypeId`，保证跨 DLL 一致。
pub trait EventRegistry: Send + Sync {
    fn register_event(
        &self,
        event_name: &'static str,
        handle: PluginHandle,
        handler: ErasedHandler,
    );
}

/// `EventRegistry` 的泛型扩展 —— 提供类型安全的 `register::<E>()`。
pub trait EventRegistryExt {
    fn register<E: Event>(&self, handle: PluginHandle, handler: Box<dyn EventHandler<E>>);
}

impl<T: EventRegistry + ?Sized> EventRegistryExt for T {
    fn register<E: Event>(&self, handle: PluginHandle, handler: Box<dyn EventHandler<E>>) {
        let expected = std::any::type_name::<E>();
        let handler: std::sync::Arc<dyn EventHandler<E>> = std::sync::Arc::from(handler);
        let erased: ErasedHandler = Box::new(
            move |host: &dyn HostApi,
                  actual: &'static str,
                  raw: &(dyn std::any::Any + Send + Sync),
                  ctx: &EventContext| {
                // type_name 跨 DLL 一致，替代 TypeId 比较
                assert_eq!(
                    actual, expected,
                    "event type mismatch: expected {expected}, got {actual}"
                );
                // SAFETY: type_name 匹配 + 同版本同源码编译 → 布局相同
                let event: &E = unsafe { &*(raw as *const dyn std::any::Any as *const E) };
                let handler = handler.clone();
                let fut = async move { handler.handle(host, event, ctx).await };
                Box::pin(fut)
            },
        );
        self.register_event(std::any::type_name::<E>(), handle, erased);
    }
}

/// 事件触发接口（dyn 兼容，放入 HostApi）。
pub trait EventBusApi: Send + Sync {
    fn emit_event<'a>(
        &'a self,
        event_name: &'static str,
        event: &'a (dyn std::any::Any + Send + Sync),
    ) -> Pin<Box<dyn Future<Output = EventEmitResult> + Send + 'a>>;
}

/// `EventBusApi` 的泛型扩展 —— 插件可调用 `api.emit(&event).await`。
pub trait EventBusExt {
    fn emit<'a, E: Event>(
        &'a self,
        event: &'a E,
    ) -> Pin<Box<dyn Future<Output = EventEmitResult> + Send + 'a>>;
}

impl<T: EventBusApi + ?Sized> EventBusExt for T {
    fn emit<'a, E: Event>(
        &'a self,
        event: &'a E,
    ) -> Pin<Box<dyn Future<Output = EventEmitResult> + Send + 'a>> {
        self.emit_event(std::any::type_name::<E>(), event)
    }
}

/// 组合 trait —— 插件拿到 `&dyn HostApi`。
pub trait HostApi:
    LogApi + ConfigApi + PluginRefApi + RuntimeApi + EventRegistry + EventBusApi
{
}

// ---------------------------------------------------------------------------
// Plugin trait — 每个插件必须实现
// ---------------------------------------------------------------------------

/// 插件生命周期接口。
///
/// # 生命周期顺序
/// 1. `on_load`   — 宿主加载动态库后立即调用（async）
/// 2. `on_enable` — 所有依赖都已就绪后调用（async）
/// 3. `on_disable` — 宿主关闭或插件被卸载时调用（sync）
///
/// 插件还需要实现 `as_any()` 以及 `fn meta()`。
#[async_trait]
pub trait Plugin: Any + Send + Sync {
    /// 返回元数据。
    fn meta(&self) -> &PluginMeta;

    /// 用于跨插件下转换。
    fn as_any(&self) -> &dyn Any;

    /// 插件被加载（库已加载，但尚未启用）—— 异步，可以 await。
    async fn on_load(&mut self, _api: &dyn HostApi) -> Result<(), PluginError> {
        Ok(())
    }

    /// 插件被启用 —— 异步，可以 await。
    async fn on_enable(&mut self, _api: &dyn HostApi) -> Result<(), PluginError> {
        Ok(())
    }

    /// 插件被禁用 / 卸载 —— 同步，清理资源。
    fn on_disable(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 入口函数类型 & 导出宏
// ---------------------------------------------------------------------------

/// 动态库导出的入口函数签名。
///
/// 宿主调用此函数获取 `Box<dyn Plugin>` 的所有权。
///
/// 注意：`dyn Plugin` 不是 C-ABI 兼容的类型。这要求宿主与插件使用
/// 完全相同的 Rust 工具链编译。在此约束下，跨动态库边界传递 trait object
/// 是可行的。
#[allow(improper_ctypes_definitions)]
pub type PluginEntryFn = unsafe extern "C" fn() -> *mut dyn Plugin;

/// 声明一个插件类型并导出入口符号。
///
/// `$plugin_type` 必须实现 `Plugin` + `Default`。
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn __qexed_plugin_create() -> *mut dyn $crate::Plugin {
            let plugin: $plugin_type = <$plugin_type as Default>::default();
            let boxed: Box<dyn $crate::Plugin> = Box::new(plugin);
            Box::into_raw(boxed)
        }
    };
}
