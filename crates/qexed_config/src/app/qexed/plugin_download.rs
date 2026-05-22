use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct PluginDownload {
    #[AutoDoc(
        key = "config.qexed.plugin_download.enable",
        migration_notice = "config.qexed.plugin_download.migration_notice.enable"
    )]
    pub enable: bool,

    #[AutoDoc(
        key = "config.qexed.plugin_download.download",
        warning = "config.qexed.plugin_download.warning.download"
    )]
    pub download: String,

    #[AutoDoc(
        key = "config.qexed.plugin_download.download_token",
        warning = "config.qexed.plugin_download.warning.download_token"
    )]
    pub download_token: String,
}

impl Default for PluginDownload {
    fn default() -> Self {
        Self {
            enable: false,
            download: "https://api.example.com/plugins/".to_owned(),
            download_token: nanoid::nanoid!(),
        }
    }
}
