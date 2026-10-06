//! 值完整性防护（防内存串改，例：CE 修改器把玩家钻石写死 64）。
//!
//! 威胁模型：
//! 1. 服务端进程内存被外部工具（Cheat Engine 等）定位并改写关键数值。
//! 2. 客户端伪造物品包（创造槽/容器点击携带非法 NBT 物品）。
//!
//! 防护设计：
//! - `Guarded<T>`：值 + HMAC-SHA256 影子签。每次读都重算比对；密钥为进程
//!   启动时随机生成的 32 字节（外部无法获知 → 无法同步伪造影子）。
//! - `ItemSignature`：服务端签发的贵重物品签名（Slot 序列化 + 密钥 HMAC 截断）。
//!   客户端上报的物品若无合法签名 → 判定非法注入。
//! - `audit`：周期对账器（快照 vs 影子 + 业务不变量），差异上报。

pub mod error;
pub mod guarded;
pub mod item_signature;
pub mod audit;

pub use audit::{AuditEvent, AuditReport, Auditor};
pub use error::{IntegrityError, Result, ViolationKind};
pub use guarded::Guarded;
pub use item_signature::ItemSignature;

/// 进程级主密钥（首次访问生成；每进程唯一——重启后旧影子全部失效，
/// 外部持久化工具无法跨进程预计算）。
pub fn master_key() -> &'static [u8; 32] {
    use std::sync::OnceLock;
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    })
}

#[cfg(test)]
mod tests;
