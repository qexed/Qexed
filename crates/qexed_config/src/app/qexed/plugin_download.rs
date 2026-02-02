use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginDownload {
    // 是否启用插件下载功能
    pub enable: bool,
    // 插件下载地址(用于插件配置)
    pub download: String,
    // 插件下载认证token
    pub download_token: String,
}
impl Default for PluginDownload {
    fn default() -> Self {
        Self {
            enable: false,
            download: "https://api.example.com/plugins/".to_owned(),
            download_token: "123456".to_owned(),
        }
    }
}
