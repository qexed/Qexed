use rust_i18n::t;
use std::net::SocketAddr;
use crate::engine::Engine;

pub struct SimpleEngine {
    config: qexed_config::app::qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
    kv: tinykv::TinyKV,
}

impl SimpleEngine {
    pub fn new(
        config: qexed_config::app::qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
    ) -> Self {
        Self {
            config,
            kv: tinykv::TinyKV::new(),
        }
    }

    /// 检查IP是否在白名单中
    fn is_whitelisted(&self, socket_addr: &SocketAddr) -> bool {
        let ip_str = socket_addr.ip().to_string();
        self.config
            .whitelist_ips
            .iter()
            .any(|whitelist_ip| whitelist_ip == &ip_str)
    }
}

#[async_trait::async_trait]
impl Engine for SimpleEngine {
    async fn check_ip(&mut self, socket_addr: std::net::SocketAddr) -> anyhow::Result<()> {
        // 1. 检查白名单
        if self.is_whitelisted(&socket_addr) {
            return Ok(());
        }
        match self.kv.get::<usize>(&socket_addr.ip().to_string())? {
            Some(v) => {
                if v>self.config.violation_count{
                    return Err(anyhow::anyhow!("{}",t!("qexed_ip_connection_speed_test.connect_to_speek",ip=socket_addr.ip().to_string())));
                }
                self.kv
                    .set_with_ttl(&socket_addr.ip().to_string(), v+1, self.config.ttl)?;
            }
            None => {
                self.kv
                    .set_with_ttl(&socket_addr.ip().to_string(), 1, self.config.ttl)?;
            }
        };
        Ok(())
    }
}
