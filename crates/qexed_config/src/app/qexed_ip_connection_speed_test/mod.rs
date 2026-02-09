// IP速率检测
pub mod data;
use serde::{Deserialize, Serialize};
use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct QexedIpConnectionSpeedTest {
    pub version: i32,
    // 你即便不启用模块,模块也会启动的,只不过他不会检测而已
    pub enable:bool,
    pub data:data::Data,
    pub ttl:u64,
    // ttl时间内的次数限制
    pub violation_count:usize,
    // 白名单
    pub whitelist_ips: Vec<String>,
}

impl Default for QexedIpConnectionSpeedTest {
    fn default() -> Self {
        Self { 
            version: Default::default(),
            enable:true,
            data:Default::default(),
            ttl:30,
            violation_count:5,
            whitelist_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
        }
    }
}

impl AppConfigTrait for QexedIpConnectionSpeedTest {
    const PATH: &'static str = "./config/";

    const NAME: &'static str = "qexed_ip_connect_speed_test";
}
