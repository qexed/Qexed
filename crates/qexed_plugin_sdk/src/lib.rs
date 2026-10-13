//! qexed v6 插件 SDK：原生 cdylib 插件的引导与便捷层。
//!
//! v4 的 `qexed_plugin_sdk` 是 WASM C-ABI import 包装；v6 改为原生动态库，
//! 本 crate 按原生模型重写。对照关系：
//!
//! | v4（wasm32，export/free fn）          | v6（原生，trait/闭包）                    |
//! |---------------------------------------|-------------------------------------------|
//! | `qexed_plugin_manifest!`             | `plugin_meta!` / `PluginMeta`            |
//! | export `<event>(ptr,len)` 订阅        | `ctx.register::<E>(closure)`             |
//! | `storage_*` 等自由函数                | [`api::Sdk`] 方法（委托宿主服务 trait）  |
//! | `world_set_block(...)`               | `Sdk::world_set_block(...)`              |
//! | `entity_upsert(&RuntimeEntity)`      | `Sdk::entity_upsert(&RuntimeEntitySpec)` |
//! | `PluginCommandDefinition`（manifest）| `ctx.register_command("menu")`           |
//!
//! 最小示例：
//!
//! ```ignore
//! qexed_plugin_sdk::declare_simple_plugin!("hello", "Hello", "0.1.0", |ctx| Box::pin(async move {
//!     let api = ctx.api();
//!     ctx.register_command("hello"); // /hello → ChatCommandEvent
//!     ctx.register::<qexed_plugin_api::PlayerJoinEvent>(|api, ev, _| Box::pin(async move {
//!         api.log(qexed_plugin_api::LogLevel::Info, &format!("欢迎 {}", ev.player.username));
//!     }));
//! }));
//! ```
//!
//! 需要取消事件或复杂状态的插件直接实现 [`qexed_plugin_api::Plugin`] +
//! [`qexed_plugin_api::EventHandler`]（见 crates/plugins/sample），本 SDK 只收敛样板。

pub mod actions;
pub mod api;
pub mod context;
pub mod events;

pub use actions::*;
pub use api::Sdk;
pub use context::{
    ChatFuture, ClosureHandler, InitFuture, PluginContext, SimpleHandler, SimplePlugin,
    plugin_meta,
};
pub use qexed_plugin_api::payload;
pub use qexed_plugin_api::services::{PlaceholderResolver, PluginServiceProvider};
pub use qexed_plugin_api::{
    declare_plugin, Event, EventBusExt, EventContext, EventHandler, EventEmitResult,
    EventRegistryExt, HostApi, LogLevel, Plugin, PluginError, PluginHandle, PluginMeta,
};
