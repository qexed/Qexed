//! qexed_forge：接受 Minecraft Forge 客户端所需的线协议支持。
//!
//! 覆盖 FML（Forge ModLoader）登录握手（`forge:handshake` 登录插件通道）：
//! 服务器在 LOGIN 阶段发出 [`login_plugin_request`](handshake)，客户端回复
//! mod 列表，服务器回接受的 mod 列表，双方 Ack 后进入正常登录流程。
//!
//! 同时提供 CONFIGURATION 阶段 Forge 通道的编解码（`forge:login` 包装、
//! `forge:channeldata` 等）。
//!
//! 许可：本 crate 为 MIT，独立于 Forge 本体（LGPLv3）实现的干净房间
//! 重新实现——只实现线上的字节格式，不包含任何 Forge 代码。

pub mod handshake;
pub mod channels;
pub mod error;

pub use error::ForgeError;
pub use handshake::{ModEntry, ModList};