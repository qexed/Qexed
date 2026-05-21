pub mod data;

use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

#[qexed_config_macros::app_config("/", "qexed_ip_connect_speed_test")]
#[derive(Debug, Serialize, Deserialize, Clone, AutoDoc)]
pub struct QexedIpConnectionSpeedTest {
    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.version")]
    pub version: i32,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.enable")]
    pub enable: bool,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data", sub)]
    pub data: data::Data,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.ttl")]
    pub ttl: u64,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.violation_count")]
    pub violation_count: usize,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.whitelist_ips")]
    pub whitelist_ips: Vec<String>,
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
