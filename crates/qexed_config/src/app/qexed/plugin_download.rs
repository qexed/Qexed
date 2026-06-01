use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginDownload {
    pub enable: bool,

    pub download: String,

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
