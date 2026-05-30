use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::plugin_download::PluginDownload;

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct QexedPluginDownload {
    #[AutoDoc(key = "config.qexed.plugin_download", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.plugin_download",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}
