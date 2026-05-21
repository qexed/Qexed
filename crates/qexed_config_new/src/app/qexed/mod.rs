use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

pub mod plugin_download;
pub mod qexed_args;
pub mod server;

#[qexed_config_macros::app_config("/", "qexed")]
#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct Qexed {
    #[AutoDoc(key = "config.qexed.version")]
    pub version: i32,

    #[AutoDoc(key = "config.qexed.update_check")]
    pub update_check: bool,

    #[AutoDoc(key = "config.qexed.language")]
    pub language: String,

    #[AutoDoc(key = "config.qexed.plugin_download", sub)]
    pub plugin_download: plugin_download::PluginDownload,

    #[AutoDoc(key = "config.qexed.server", sub)]
    pub server: server::Server,
}

impl Qexed {
    fn get_system_language() -> String {
        match sys_locale::get_locale() {
            Some(v) => v,
            None => {
                log::warn!(
                    "检测当前系统语言失败,使用默认语言 zh-CN,您可以手动修改配置文件 config/qexed.toml中的 language 来设置语言"
                );
                "zh-CN".to_string()
            }
        }
    }
}

impl Default for Qexed {
    fn default() -> Self {
        Self {
            version: Default::default(),
            update_check: true,
            plugin_download: Default::default(),
            server: Default::default(),
            language: Qexed::get_system_language(),
        }
    }
}
