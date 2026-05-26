use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

pub mod plugin_download;
pub mod qexed_args;
pub mod server;

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

impl qexed_config::tool::AppConfigTrait for Qexed {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed";

    fn split_config_files() -> &'static [qexed_config::tool::SplitConfigFile] {
        &[
            qexed_config::tool::SplitConfigFile {
                file_name: "plugin_download.toml",
                root_path: "plugin_download",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "server.toml",
                root_path: "server",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "world.toml",
                root_path: "server.world",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "lan_discovery.toml",
                root_path: "server.lan_discovery",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "player_data.toml",
                root_path: "server.player_data",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "player_messages.toml",
                root_path: "server.player_messages",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "content_filter.toml",
                root_path: "server.content_filter",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "permissions.toml",
                root_path: "server.permissions",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "resource_pack.toml",
                root_path: "server.resource_pack",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "entities.toml",
                root_path: "server.entities",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "scoreboard.toml",
                root_path: "server.scoreboard",
            },
            qexed_config::tool::SplitConfigFile {
                file_name: "lobby.toml",
                root_path: "server.lobby",
            },
        ]
    }
}
