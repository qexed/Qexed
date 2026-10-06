//! qexed_plugins：插件系统（v4 qexed/src/plugins.rs + 独立 crate qexed_plugin_api 迁移）。
//!
//! v6 加载机制：libloading 动态库（Windows dll / Unix so），替代 v4 wasmtime WASM。
//! 模块结构：
//! - api：v4 qexed_plugin_api（事件 / payload / 占位符文档）
//! - manager：PluginManager（加载、拓扑排序、事件分发）
//! - host：宿主服务（经济 / 存储 / 随机池 / world edit / 实体控制 / vtable）
//! - instance：动态库插件实例（C ABI 约定）
//! - economy / structured_storage：存储后端（文件实现 + TODO(storage) 接口）
//! - files：插件目录扫描（dll/so）

pub mod api;
pub mod config;
pub mod error;
pub(crate) mod economy;
mod files;
pub(crate) mod host;
pub(crate) mod instance;
pub mod manager;
pub(crate) mod structured_storage;

pub use manager::{PluginManager, PlayerBlockHitPayload};
