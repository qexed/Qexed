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
        split_dir: &std::path::Path,
    ) -> anyhow::Result<Vec<qexed_config::tool::OwnedSplitConfigFile>> {
        let mut split_files = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        if let Some(server_item) = doc.get("server").and_then(|item| item.as_table()) {
            for (key, value) in server_item.iter() {
                if key.is_empty() || !value.is_table_like() {
                    continue;
                }
                if key == "world" {
                    continue;
                }
                let file_name = format!("{key}.toml");
                let root_path = format!("server.{key}");
                seen.insert(root_path.clone());
                split_files.push(qexed_config::tool::OwnedSplitConfigFile {
                    file_name,
                    root_path,
                });
            }
        }
        if split_dir.exists() {
            for entry in std::fs::read_dir(split_dir)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if stem.eq_ignore_ascii_case("server")
                    || stem.eq_ignore_ascii_case("plugin_download")
                {
                    continue;
                }
                if stem.eq_ignore_ascii_case("world") {
                    continue;
                }
                if stem.trim().is_empty() {
                    continue;
                }
                let root_path = format!("server.{stem}");
                if !seen.insert(root_path.clone()) {
                    continue;
                }
                split_files.push(qexed_config::tool::OwnedSplitConfigFile {
                    file_name: format!("{stem}.toml"),
                    root_path,
                });
            }
        }
        Ok(split_files)
    }
}

#[cfg(test)]
mod tests {
    use super::Qexed;
    use qexed_config::tool::AppConfigTrait;

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

    #[test]
    fn dynamic_split_files_can_be_discovered_from_split_directory() {
        let temp = std::env::temp_dir().join(format!(
            "qexed-config-split-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("world.toml"), "path=\"world\"\n").unwrap();
        std::fs::write(
            temp.join("player_data.toml"),
            "[server.player_data]\nenable=true\n",
        )
        .unwrap();

        let doc = toml_edit::DocumentMut::new();
        let files = Qexed::dynamic_split_config_files(&doc, &temp).unwrap();
        let roots = files
            .into_iter()
            .map(|file| file.root_path)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(!roots.contains("server.world"));
        assert!(roots.contains("server.player_data"));
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn load_or_create_default_overlays_dynamic_split_files() {
        let temp = std::env::temp_dir().join(format!(
            "qexed-config-load-split-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let split_dir = temp.join("qexed.d");
        std::fs::create_dir_all(&split_dir).unwrap();
        std::fs::write(
            temp.join("qexed.toml"),
            r#"
version = 0
update_check = true
language = "zh-CN"
"#,
        )
        .unwrap();
        std::fs::write(
            split_dir.join("menus.toml"),
            r#"
[menus]
enable = true
reset_inventory_on_join = true
fixed_slots_only = true

[[menus.hotbar_items]]
slot = 4
item = "minecraft:compass"
name = "Menu"

[menus.hotbar_items.action]
kind = "open_menu"
target = "main"

[[menus.chests]]
id = "main"
title = "Server Menu"
rows = 3
"#,
        )
        .unwrap();
        std::fs::write(
            split_dir.join("entity_rendering.toml"),
            r#"
[entity_rendering]
default_distance = 40.0
player_distance = 24.0
npc_distance = 48.0
hologram_distance = 80.0
item_distance = 12.0
stack_threshold = 7
stack_radius = 3.5
"#,
        )
        .unwrap();

        let config = Qexed::load_or_create_default(
            Some("zh-CN".to_string()),
            Some(false),
            Some(temp.clone()),
        )
        .unwrap();

        assert!(config.server.menus.enable);
        assert_eq!(config.server.menus.hotbar_items[0].slot, 4);
        assert_eq!(config.server.menus.chests[0].id, "main");
        assert_eq!(config.server.entity_rendering.player_distance, 24.0);
        assert_eq!(config.server.entity_rendering.item_distance, 12.0);
        assert_eq!(config.server.entity_rendering.stack_threshold, 7);
        assert_eq!(config.server.entity_rendering.stack_radius, 3.5);
        let _ = std::fs::remove_dir_all(&temp);
    }
}
