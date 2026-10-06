//! 玩家统计系统（原版 Statistics 页面的服务端支持）。
//!
//! 数据面：
//! - `StatKey`：typed（mined/crafted/used/... + 注册表条目）与 custom（78 预置键）
//! - `StatsCounter`：每玩家计数器（add/set/snapshot），线程安全
//! - `CUSTOM_*` 常量与 `ALL_CUSTOM`：与 26.3 custom_stat 注册表一致
//!
//! 协议面：AwardStats（0x03）——登录/变化时推送，原版客户端统计页渲染数据源。
//! 存储面：SerializedStats ↔ PlayerData NBT（stats 标签）。

pub mod error;
pub mod model;
pub mod registry;
pub mod counter;
pub mod packet;
pub mod persist;

pub use counter::StatsCounter;
pub use error::{Result, StatisticsError};
pub use model::{StatKey, StatValue, TypedStat, TypedStatKind};
pub use registry::{ALL_CUSTOM, CUSTOM_PLAY_TIME, StatTypes};

#[cfg(test)]
mod tests;
