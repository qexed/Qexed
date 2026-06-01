use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{Server, ServerLogLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedServer {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedServerCore {
    pub ip: String,

    pub online: bool,

    pub max_player: i32,

    pub display_players: bool,

    pub online_mode: bool,

    #[serde(default)]
    pub log_level: ServerLogLevel,

    #[serde(default = "default_mojang_cache_path")]
    pub mojang_cache_path: String,

    pub network_compression_threshold: isize,

    pub max_port_connections: u16,

    pub rate_limit_window_secs: u64,

    pub rate_limit_max_attempts: u32,

    #[serde(default)]
    pub click_detection: crate::app::qexed::server::ClickDetection,

    pub motd: Vec<String>,

    #[serde(default)]
    pub code_of_conduct: bool,

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
        server.max_port_connections = self.max_port_connections;
        server.rate_limit_window_secs = self.rate_limit_window_secs;
        server.rate_limit_max_attempts = self.rate_limit_max_attempts;
        server.click_detection = self.click_detection;
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
            max_port_connections: server.max_port_connections,
            rate_limit_window_secs: server.rate_limit_window_secs,
            rate_limit_max_attempts: server.rate_limit_max_attempts,
            click_detection: server.click_detection,
            motd: vec![
                "qexed服务端awa".to_string()
            ],
            code_of_conduct: server.code_of_conduct,
            favicon: server.favicon,
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedServer {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_server";

    fn obsolete_root_paths() -> &'static [&'static str] {
        &[
            "server.proxy",
            "server.proxy_protocol",
            "server.proxy_server_id",
            "server.proxy_token",
        ]
    }
}

fn default_mojang_cache_path() -> String {
    "cache/mojang".to_string()
}
