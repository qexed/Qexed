use serde::{Deserialize, Serialize};

use crate::app::qexed::server::ResourcePack;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedResourcePack {
    pub resource_pack: ResourcePack,
}

impl Default for QexedResourcePack {
    fn default() -> Self {
        Self {
            resource_pack: ResourcePack::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedResourcePack {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_resource_pack";
}
