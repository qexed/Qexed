use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{ForwardingMode, Server};

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedProxy {
    #[AutoDoc(key = "config.qexed.proxy", sub)]
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

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct Proxy {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.proxy.enable")]
    pub enable: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.proxy.protocol")]
    pub protocol: ForwardingMode,

    #[serde(default)]
    #[AutoDoc(
        key = "config.qexed.proxy.server_id",
        warning = "config.qexed.proxy.warning.server_id"
    )]
    pub server_id: String,

    #[serde(default = "default_proxy_token")]
    #[AutoDoc(
        key = "config.qexed.proxy.token",
        warning = "config.qexed.proxy.warning.token",
        sensitive,
        default_display = "<stored in .secrets>"
    )]
    pub token: String,

    #[serde(default = "default_proxy_online_mode")]
    #[AutoDoc(key = "config.qexed.proxy.online_mode")]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.proxy",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

fn default_proxy_token() -> String {
    nanoid::nanoid!()
}

fn default_proxy_online_mode() -> bool {
    true
}
