//! 各阶段读超时与 Configuration 阶段 KeepAlive 调度参数。

use std::time::Duration;

pub(crate) const SERVER_BRAND: &str = "qexed";
pub(crate) const DEFAULT_DISPLAYED_SKIN_PARTS: u8 = 0x7f;

// ── 各阶段读超时 ─────────────────────────────────────────
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const STATUS_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const CONFIG_READ_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const SESSION_VERIFY_TIMEOUT: Duration = Duration::from_secs(15);

// ── Configuration 阶段 KeepAlive 调度参数 ────────────────
// vanilla 默认 15s 发一次，30s 未回应判超时。
pub(crate) const CONFIG_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
pub(crate) const CONFIG_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(30);