use serde::{Deserialize, Serialize};

use crate::app::qexed::server::LanDiscovery;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedLanDiscovery {
    pub lan_discovery: LanDiscovery,
}

impl Default for QexedLanDiscovery {
    fn default() -> Self {
        Self {
            lan_discovery: LanDiscovery::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedLanDiscovery {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_lan_discovery";
}
