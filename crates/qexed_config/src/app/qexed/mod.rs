use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

pub mod plugin_download;
pub mod qexed_args;
pub mod server;
pub mod world_rules;

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
    fn normalize_system_language(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let lowered = trimmed.to_ascii_lowercase();
        if lowered == "c" || lowered == "posix" {
            return None;
        }
        Some(trimmed.replace('_', "-"))
    }

    fn get_system_language() -> String {
        if let Some(locale) =
            sys_locale::get_locale().and_then(|value| Self::normalize_system_language(&value))
        {
            return locale;
        }
        log::warn!(
            "检测当前系统语言失败，使用默认语言 zh-CN。您可以手动修改配置文件 config/qexed.toml 中的 language 来设置语言。"
        );
        "zh-CN".to_string()
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
        ]
    }

    fn dynamic_split_config_files(
        doc: &toml_edit::DocumentMut,
        _split_dir: &std::path::Path,
    ) -> anyhow::Result<Vec<qexed_config::tool::OwnedSplitConfigFile>> {
        let mut split_files = Vec::new();
        if let Some(server_item) = doc.get("server").and_then(|item| item.as_table()) {
            for (key, value) in server_item.iter() {
                if key.is_empty() || !value.is_table_like() {
                    continue;
                }
                split_files.push(qexed_config::tool::OwnedSplitConfigFile {
                    file_name: format!("{key}.toml"),
                    root_path: format!("server.{key}"),
                });
            }
        }
        Ok(split_files)
    }
}

#[cfg(test)]
mod tests {
    use super::Qexed;

    #[test]
    fn normalize_system_language_filters_invalid_locale() {
        assert_eq!(
            Qexed::normalize_system_language("zh_CN"),
            Some("zh-CN".to_string())
        );
        assert_eq!(
            Qexed::normalize_system_language("en-US"),
            Some("en-US".to_string())
        );
        assert_eq!(Qexed::normalize_system_language("C"), None);
        assert_eq!(Qexed::normalize_system_language("POSIX"), None);
        assert_eq!(Qexed::normalize_system_language("  "), None);
    }
}
