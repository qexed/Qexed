use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::LanDiscovery;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedLanDiscovery {
    #[AutoDoc(key = "config.qexed.server.lan_discovery", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.lan_discovery",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}
