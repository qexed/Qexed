use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
    pub version_manifest_url: String,
    pub cache_dir: String,
    pub data_marker: String,
    pub http_timeout: u64,
}

impl Registry {}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version_manifest_url: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
                .to_string(),
            cache_dir: "cache/mojang".to_string(),
            data_marker: ".qexed-data-ready".to_string(),
            http_timeout: 120,
        }
    }
}

impl qexed_config::tool::AppConfigTrait for Registry {
    const PATH: &'static str = "/";
    const NAME: &'static str = "registry";
}
