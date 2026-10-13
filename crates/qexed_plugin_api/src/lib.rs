//! qexed 插件 ABI/API：宿主与动态库插件共享的唯一契约 crate。
//!
//! # 加载模型
//!
//! 插件是 cdylib（Windows 上 .dll，Linux 上 .so，macOS 上 .dylib），
//! 通过 [`declare_plugin!`] 导出入口符号 `__qexed_plugin_create`；宿主用
//! `libloading` 加载后取得 `Box<dyn Plugin>` 所有权。
//!
//! **兼容性约束**：`dyn Plugin` 不是 C-ABI 兼容类型，要求宿主与插件使用
//! 完全相同的 qexed_plugin_api 版本 + 同版本 Rust 工具链编译（与 v5 一致）。
//!
//! # 事件
//!
//! 事件按 `std::any::type_name` 字符串分发（跨动态库 TypeId 不保证一致，
//! type_name 字符串一致）；插件用 [`EventRegistryExt::register`] 订阅，
//! 宿主与插件用 [`EventBusExt::emit`] 触发。
//!
//! # 日志
//!
//! 插件不要直接依赖具体日志门面，统一走 [`LogApi`]（由宿主转发到
//! 服务端日志系统），保证插件日志带插件上下文且可集中过滤。

pub mod events;
pub mod payload;
pub mod services;

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
// LogLevel / LogApi
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// 日志接口。
pub trait LogApi: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
}

// ---------------------------------------------------------------------------
// ConfigApi
// ---------------------------------------------------------------------------

/// 配置读写接口。
///
/// 文件路径为 `config/<plugin_id>/config.toml`。
pub trait ConfigApi: Send + Sync {
    fn read_config_toml(&self, plugin_id: &str) -> Result<Option<String>, PluginError>;
    fn write_config_toml(&self, plugin_id: &str, toml_content: &str) -> Result<(), PluginError>;
}

// ---------------------------------------------------------------------------
// PluginHandle / PluginRefApi
// ---------------------------------------------------------------------------

/// 插件整数句柄，等价于数组下标，O(1) 零开销查找。
///
/// 在 `on_enable` 阶段通过 `api.get_plugin_handle("id")` 获取依赖插件的
/// handle，存入自己的结构体，后续热路径直接用 `api.get_plugin(handle)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginHandle(pub usize);

/// 跨插件引用接口。
pub trait PluginRefApi: Send + Sync {
    /// 通过句柄获取插件实例 — 热路径，O(1) 数组下标 + Arc 克隆。
    ///
    /// 返回克隆而非引用：插件表在锁内，引用无法安全外借。
    /// 依赖方拿到 Arc 后用 `as_any()` 下转换到具体类型。
    fn get_plugin(&self, handle: PluginHandle) -> Option<std::sync::Arc<dyn Plugin>>;

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
// 事件模型
// ---------------------------------------------------------------------------

/// 事件标记 trait —— 每个事件类型实现它。
pub trait Event: Send + Sync + 'static {}

/// 单次事件分发的上下文。
///
/// handler 可以通过 [`EventContext::cancel`](EventContext::cancel) 标记事件
/// 已取消；触发方通过 [`EventEmitResult`] 读取最终状态。
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
/// 跨动态库边界安全 —— 闭包由插件（动态库）创建，fire 时直接调用，无需 Any downcast。
///
/// `for<'a>` 让返回的 future 能借用入参 `host` / `raw` 的生命周期。
pub type ErasedHandler = Box<
    dyn for<'a> Fn(
            &'a dyn HostApi,
            &'static str,
            &'a (dyn Any + Send + Sync),
            &'a EventContext,
        ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'a>>
        + Send
        + Sync,
>;

/// 事件注册接口（dyn 兼容，放入 HostApi）。
///
/// 使用 `type_name` 字符串代替 `TypeId`，保证跨动态库一致。
pub trait EventRegistry: Send + Sync {
    fn register_event(
        &self,
        event_name: &'static str,
        handle: PluginHandle,
        handler: ErasedHandler,
    );
}

/// [`EventRegistry`] 的泛型扩展 —— 提供类型安全的 `register::<E>()`。
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
                  raw: &(dyn Any + Send + Sync),
                  ctx: &EventContext| {
                // type_name 跨动态库一致，替代 TypeId 比较
                assert_eq!(
                    actual, expected,
                    "event type mismatch: expected {expected}, got {actual}"
                );
                // SAFETY: type_name 匹配 + 同版本同源码编译 → 布局相同
                let event: &E = unsafe { &*(raw as *const dyn Any as *const E) };
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
        event: &'a (dyn Any + Send + Sync),
    ) -> Pin<Box<dyn Future<Output = EventEmitResult> + Send + 'a>>;
}

/// [`EventBusApi`] 的泛型扩展 —— 插件可调用 `api.emit(&event).await`。
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

// ---------------------------------------------------------------------------
// 事件（v4 全量语义，定义见 events.rs；此处 re-export 为 crate 根路径）
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// 玩家信息快照（事件/占位符共用的轻量只读结构）
// ---------------------------------------------------------------------------

/// 玩家信息快照。
///
/// `entity_id` 是该玩家当前连接的服务端实体 id（v4 语义：动作自指用）。
/// `dimension` 形如 `minecraft:overworld`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    /// 玩家 UUID（带横线小写形式）。
    pub uuid: String,
    /// 玩家名。
    pub username: String,
    /// 服务端实体 id。
    pub entity_id: i32,
    /// 当前维度命名空间 id。
    pub dimension: String,
}

impl PlayerInfo {
    /// 构造玩家信息快照。
    pub fn new(uuid: impl Into<String>, username: impl Into<String>, entity_id: i32, dimension: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            username: username.into(),
            entity_id,
            dimension: dimension.into(),
        }
    }
}

pub use events::*;
pub use payload::*;


// ---------------------------------------------------------------------------
// HostApi
// ---------------------------------------------------------------------------

/// 组合 trait —— 插件在生命周期回调里拿到 `&dyn HostApi`。
pub trait HostApi:
    LogApi
    + ConfigApi
    + PluginRefApi
    + RuntimeApi
    + EventRegistry
    + EventBusApi
    + services::WorldEditApi
    + services::EntityControlApi
    + services::StructuredStorageApi
    + services::EconomyApi
    + services::RandomPoolApi
    + services::PlaceholderApi
    + services::HttpClientApi
    + services::PathfindingApi
    + services::GeyserApi
    + services::InterPluginApi
    + services::PlayerActionApi
    + services::CommandApi
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
///
/// 所有生命周期回调都接收 `&self`：插件实例在宿主侧以 `Arc` 共享，
/// 运行期可能被并发触发事件。需要内部可变性的插件自行使用
/// `Mutex` / `RwLock` / `OnceLock` 管理状态。
#[async_trait]
pub trait Plugin: Any + Send + Sync {
    /// 返回元数据。
    fn meta(&self) -> &PluginMeta;

    /// 用于跨插件下转换。
    fn as_any(&self) -> &dyn Any;

    /// 插件被加载（库已加载，但尚未启用）—— 异步，可以 await。
    async fn on_load(&self, _api: &dyn HostApi) -> Result<(), PluginError> {
        Ok(())
    }

    /// 插件被启用 —— 异步，可以 await。
    async fn on_enable(&self, _api: &dyn HostApi) -> Result<(), PluginError> {
        Ok(())
    }

    /// 插件被禁用 / 卸载 —— 同步，清理资源。
    fn on_disable(&self) -> Result<(), PluginError> {
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