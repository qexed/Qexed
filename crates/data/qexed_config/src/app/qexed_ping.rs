use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct PingConfig {
    pub version: i32,
    ///  Ping 间隔（秒），默认 5
    pub interval: i32,
    ///  最大重试次数，默认 3
    pub max_retries: i32,
    /// 启用延迟限制功能
    pub enable_latency_limit: bool,
    /// 高延迟限制（毫秒），超过此值触发限制
    pub latency_limit_ms: i32,
}
impl Default for PingConfig {
    fn default() -> Self {
        Self {
            version: 0,
            interval: 5,
            max_retries: 3,
            enable_latency_limit: true,
            latency_limit_ms: 300,
        }
    }
}
impl AppConfigTrait for PingConfig {
    const PATH: &'static str = "./config/qexed_ping/";
    const NAME: &'static str = "config";
}
