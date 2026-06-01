use serde::{Deserialize, Serialize};

use crate::app::qexed::plugin_download::PluginDownload;

#[derive(Debug, Serialize, Deserialize)]
pub struct QexedPluginDownload {
    pub plugin_download: PluginDownload,
}

impl Default for QexedPluginDownload {
    fn default() -> Self {
        Self {
            plugin_download: PluginDownload::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPluginDownload {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_plugin_download";
}
