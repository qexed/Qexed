use serde::{Deserialize, Serialize};
use crate::tool::AppConfigTrait;

#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct HeartbeatConfig {
    /// 心跳间隔（秒）
    pub interval_seconds: i32,
    
    /// 超时时间（秒）
    pub timeout_seconds: i32,
    
    /// 最大连续丢失次数
    pub max_consecutive_misses: u32,
    
    /// 是否启用心跳
    pub enabled: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 5,
            timeout_seconds: 30,
            max_consecutive_misses: 3,
            enabled: true,
        }
    }
}
impl AppConfigTrait for HeartbeatConfig {
    const PATH: &'static str = "./config/qexed_heartbeat/";
    const NAME: &'static str = "config";
}
