use qexed_config_macros::AutoDoc;
use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{ForwardingMode, Server, ServerLogLevel};

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedServer {
    #[AutoDoc(key = "config.qexed.server", sub)]
    pub server: QexedServerCore,
}

impl QexedServer {
    pub fn apply_to(self, server: &mut Server) {
        self.server.apply_to(server);
    }
}

impl Default for QexedServer {
    fn default() -> Self {
        Self {
            server: QexedServerCore::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedServerCore {
    #[AutoDoc(key = "config.qexed.server.ip")]
    pub ip: String,

    #[AutoDoc(
        key = "config.qexed.server.online",
        warning = "config.qexed.server.warning.online"
    )]
    pub online: bool,

    #[AutoDoc(key = "config.qexed.server.max_player")]
    pub max_player: i32,

    #[AutoDoc(key = "config.qexed.server.display_players")]
    pub display_players: bool,

    #[AutoDoc(
        key = "config.qexed.server.online_mode",
        warning = "config.qexed.server.warning.online_mode"
    )]
    pub online_mode: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.log_level")]
    pub log_level: ServerLogLevel,

    #[serde(default = "default_mojang_cache_path")]
    #[AutoDoc(key = "config.qexed.server.mojang_cache_path")]
    pub mojang_cache_path: String,

    #[AutoDoc(key = "config.qexed.server.network_compression_threshold")]
    pub network_compression_threshold: isize,

    #[AutoDoc(key = "config.qexed.server.proxy")]
    pub proxy: bool,

    #[AutoDoc(key = "config.qexed.server.proxy_protocol")]
    pub proxy_protocol: ForwardingMode,

    #[serde(default)]
    #[AutoDoc(
        key = "config.qexed.server.proxy_server_id",
        warning = "config.qexed.server.warning.proxy_server_id"
    )]
    pub proxy_server_id: String,

    #[AutoDoc(
        key = "config.qexed.server.proxy_token",
        warning = "config.qexed.server.warning.proxy_token",
        sensitive,
        default_display = "<stored in .secrets>"
    )]
    pub proxy_token: String,

    #[AutoDoc(key = "config.qexed.server.max_port_connections")]
    pub max_port_connections: u16,

    #[AutoDoc(key = "config.qexed.server.rate_limit_window_secs")]
    pub rate_limit_window_secs: u64,

    #[AutoDoc(key = "config.qexed.server.rate_limit_max_attempts")]
    pub rate_limit_max_attempts: u32,

    #[AutoDoc(key = "config.qexed.server.motd")]
    pub motd: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.code_of_conduct")]
    pub code_of_conduct: bool,

    #[AutoDoc(key = "config.qexed.server.favicon")]
    pub favicon: String,
}

impl QexedServerCore {
    pub fn apply_to(self, server: &mut Server) {
        server.ip = self.ip;
        server.online = self.online;
        server.max_player = self.max_player;
        server.display_players = self.display_players;
        server.online_mode = self.online_mode;
        server.log_level = self.log_level;
        server.mojang_cache_path = self.mojang_cache_path;
        server.network_compression_threshold = self.network_compression_threshold;
        server.proxy = self.proxy;
        server.proxy_protocol = self.proxy_protocol;
        server.proxy_server_id = self.proxy_server_id;
        server.proxy_token = self.proxy_token;
        server.max_port_connections = self.max_port_connections;
        server.rate_limit_window_secs = self.rate_limit_window_secs;
        server.rate_limit_max_attempts = self.rate_limit_max_attempts;
        server.motd = self.motd;
        server.code_of_conduct = self.code_of_conduct;
        server.favicon = self.favicon;
    }
}

impl Default for QexedServerCore {
    fn default() -> Self {
        let server = Server::default();
        Self {
            ip: server.ip,
            online: server.online,
            max_player: server.max_player,
            display_players: server.display_players,
            online_mode: server.online_mode,
            log_level: server.log_level,
            mojang_cache_path: server.mojang_cache_path,
            network_compression_threshold: server.network_compression_threshold,
            proxy: server.proxy,
            proxy_protocol: server.proxy_protocol,
            proxy_server_id: server.proxy_server_id,
            proxy_token: server.proxy_token,
            max_port_connections: server.max_port_connections,
            rate_limit_window_secs: server.rate_limit_window_secs,
            rate_limit_max_attempts: server.rate_limit_max_attempts,
            motd: vec![
                t!("qexed_config.config.server.motd1").to_string(),
                t!("qexed_config.config.server.motd2").to_string(),
            ],
            code_of_conduct: server.code_of_conduct,
            favicon: server.favicon,
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedServer {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_server";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.server",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}

fn default_mojang_cache_path() -> String {
    "cache/mojang".to_string()
}
