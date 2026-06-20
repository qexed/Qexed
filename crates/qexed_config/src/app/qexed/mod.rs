use serde::{Deserialize, Serialize};
pub mod args;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qexed {
    pub server: Server,
}

impl Qexed {}

impl Default for Qexed {
    fn default() -> Self {
        Self {
            server: Server::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for Qexed {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub bind: String,
    pub max_connections: usize,
    pub motd: String,
    pub max_players: i32,
    pub view_distance: i32,
    pub simulation_distance: i32,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:25565".to_string(),
            max_connections: 128,
            motd: "Qexed".to_string(),
            max_players: 20,
            view_distance: 8,
            simulation_distance: 8,
        }
    }
}
