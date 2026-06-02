use serde::{Deserialize, Serialize};

pub mod plugin_download;
pub mod qexed_args;
pub mod server;
pub mod world_rules;

#[derive(Debug, Serialize, Deserialize)]
pub struct Qexed {
    pub version: i32,

    pub update_check: bool,

    pub language: String,

    #[serde(skip)]
    pub plugin_download: plugin_download::PluginDownload,

    #[serde(skip)]
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
            "system language detection failed, falling back to zh-CN. You can set language in config/qexed.toml."
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
