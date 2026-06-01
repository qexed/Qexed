use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{ForwardingMode, Server};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedProxy {
    pub proxy: Proxy,
}

impl QexedProxy {
    pub fn apply_to(self, server: &mut Server) {
        self.proxy.apply_to(server);
    }
}

impl Default for QexedProxy {
    fn default() -> Self {
        Self {
            proxy: Proxy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub protocol: ForwardingMode,

    #[serde(default)]
    pub server_id: String,

    #[serde(default = "default_proxy_token")]
    pub token: String,

    #[serde(default = "default_proxy_online_mode")]
    pub online_mode: bool,
}

impl Proxy {
    pub fn apply_to(self, server: &mut Server) {
        server.proxy = self.enable;
        server.proxy_protocol = self.protocol;
        server.proxy_server_id = self.server_id;
        server.proxy_token = self.token;
        server.proxy_online_mode = self.online_mode;
    }
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            enable: false,
            protocol: ForwardingMode::QTunnel,
            server_id: String::new(),
            token: default_proxy_token(),
            online_mode: default_proxy_online_mode(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedProxy {
    const PATH: &'static str = "/";
    const NAME: &'static str = "proxy";
}

fn default_proxy_token() -> String {
    nanoid::nanoid!()
}

fn default_proxy_online_mode() -> bool {
    true
}
