pub mod data;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QexedIpConnectionSpeedTest {
    pub version: i32,

    pub enable: bool,

    pub data: data::Data,

    pub ttl: u64,

    pub violation_count: usize,

    pub whitelist_ips: Vec<String>,
}

impl crate::tool::AppConfigTrait for QexedIpConnectionSpeedTest {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_ip_connect_speed_test";
}

impl Default for QexedIpConnectionSpeedTest {
    fn default() -> Self {
        Self {
            version: Default::default(),
            enable: true,
            data: Default::default(),
            ttl: 30,
            violation_count: 5,
            whitelist_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
        }
    }
}
