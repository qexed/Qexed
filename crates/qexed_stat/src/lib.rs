//! 玩家统计（统计屏幕）：计数、协议 id 解析与 award_stats 包构造。
//!
//! - 协议：`minecraft:award_stats`（play s2c 0x03），内容为
//!   `(category_id, stat_id, value)` 三元组列表；原版语义是「dirty 集的绝对值」，
//!   客户端 `handleAwardStats` 用 `setValue` 直接覆盖本地缓存。
//! - 触发：客户端打开统计屏幕时发 `ClientCommand(action = REQUEST_STATS)`，
//!   服务端此时回 dirty 集——这是原版 `sendStats` 的唯一调用点，没有周期推送。
//! - id 数据源：`qexed_registry` 读缓存的 `reports/registries.json`。stat_type /
//!   custom_stat / block / item / entity_type 都是 built-in 注册表（jar bootstrap
//!   写死），不参与 configuration 阶段的 registry_data 同步。
//!
//! 屏幕三个标签页与类别的对应：通用 = custom(8)；生物 = killed(6) /
//! killed_by(7)；物品 = mined(0)/crafted(1)/used(2)/broken(3)/picked_up(4)/dropped(5)。
pub mod config;
pub mod counter;
pub mod error;
pub mod registry;
