//! Mojang 原版语言文件（en_us.json / zh_cn.json …）加载与查询。
//! 迁移自 v4 crates/qexed/src/l10n.rs；错误从 anyhow 换成 crate::error::ServerError。
//!
//! 与 qexed_language 的分工：qexed_language 管「服务器自身文案」的 i18n；
//! 本模块管「游戏内容」（方块/物品/实体/附魔显示名）的原版翻译表。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::Value;

use crate::error::{Result, ServerError};

static L10N: OnceLock<L10nData> = OnceLock::new();

#[derive(Debug, Clone)]
struct L10nData {
    translations: HashMap<String, HashMap<String, String>>,
    fallback_language: String,
}

/// 初始化 l10n 模块：lang 目录应包含 Mojang 语言 JSON 文件
/// （如 en_us.json、zh_cn.json）。
pub fn initialize(lang_dir: &Path, fallback_language: impl Into<String>) -> Result<()> {
    let fallback = fallback_language.into();
    let mut translations = HashMap::new();

    if !lang_dir.exists() {
        log::warn!(
            "{}",
            qexed_language::t("qexed.server.l10n.dir_missing").replace("%{path}", &lang_dir.display().to_string())
        );
        return L10N
            .set(L10nData {
                translations,
                fallback_language: fallback,
            })
            .map_err(|_| ServerError::msg("l10n already initialized"));
    }

    for entry in std::fs::read_dir(lang_dir).map_err(|source| ServerError::IoContext {
        context: format!("无法读取 lang 目录: {}", lang_dir.display()),
        source,
    })? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        // 加载语言 JSON 文件
        match load_lang_file(&path) {
            Ok(entries) => {
                let count = entries.len();
                translations.insert(file_name.to_string(), entries);
                log::debug!("l10n: loaded language '{file_name}' with {count} translations");
            }
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.l10n.file_failed")
                        .replace("%{path}", &path.display().to_string())
                        .replace("%{error}", &err.to_string())
                );
            }
        }
    }

    if translations.is_empty() {
        log::warn!("{}", qexed_language::t("qexed.server.l10n.no_files"));
    } else {
        log::info!(
            "{}",
            qexed_language::t("qexed.server.l10n.initialized")
                .replace("%{count}", &translations.len().to_string())
                .replace("%{fallback}", &fallback)
        );
    }

    L10N.set(L10nData {
        translations,
        fallback_language: fallback,
    })
    .map_err(|_| ServerError::msg("l10n already initialized"))
}

fn load_lang_file(path: &Path) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path).map_err(|source| ServerError::IoContext {
        context: format!("无法读取语言文件: {}", path.display()),
        source,
    })?;
    let value: Value = serde_json::from_str(&content).map_err(ServerError::Json)?;

    let obj = value
        .as_object()
        .ok_or_else(|| ServerError::msg(format!("语言文件根节点不是对象: {}", path.display())))?;

    let mut entries = HashMap::with_capacity(obj.len());
    for (key, val) in obj {
        if let Some(text) = val.as_str() {
            entries.insert(key.clone(), text.to_string());
        }
    }
    Ok(entries)
}

/// 按注册表键（如 "minecraft:stone"）取物品/方块的本地化显示名。
///
/// language 为玩家客户端语言代码（如 "zh_CN"、"en_US"）。
/// 没有可用翻译时返回 None。
pub fn localize(key: &str, language: &str) -> Option<String> {
    let data = L10N.get()?;
    let lang_code = normalize_language_code(language);

    // 先查请求的语言，再回退 fallback
    for lang in [&lang_code, &data.fallback_language] {
        if let Some(translations) = data.translations.get(lang.as_str()) {
            // 翻译键格式：block.<namespace>.<path>、item.<namespace>.<path>
            for prefix in ["block", "item"] {
                let translation_key = registry_key_to_translation_key(key, prefix);
                if let Some(text) = translations.get(&translation_key) {
                    return Some(text.clone());
                }
            }

            // 也尝试原样直接查键
            if let Some(text) = translations.get(key) {
                return Some(text.clone());
            }
        }
    }
    None
}

/// 按注册表键（如 "minecraft:creeper"）取实体类型的本地化显示名。
pub fn localize_entity(key: &str, language: &str) -> Option<String> {
    let data = L10N.get()?;
    let lang_code = normalize_language_code(language);

    for lang in [&lang_code, &data.fallback_language] {
        if let Some(translations) = data.translations.get(lang.as_str()) {
            let translation_key = registry_key_to_translation_key(key, "entity");
            if let Some(text) = translations.get(&translation_key) {
                return Some(text.clone());
            }
            if let Some(text) = translations.get(key) {
                return Some(text.clone());
            }
        }
    }
    None
}

/// 按注册表键（如 "minecraft:sharpness"）取附魔的本地化显示名。
pub fn localize_enchantment(key: &str, language: &str) -> Option<String> {
    let data = L10N.get()?;
    let lang_code = normalize_language_code(language);

    for lang in [&lang_code, &data.fallback_language] {
        if let Some(translations) = data.translations.get(lang.as_str()) {
            let translation_key = registry_key_to_translation_key(key, "enchantment");
            if let Some(text) = translations.get(&translation_key) {
                return Some(text.clone());
            }
            if let Some(text) = translations.get(key) {
                return Some(text.clone());
            }
        }
    }
    None
}

/// 按翻译键直接取原始翻译。
pub fn translate(translation_key: &str, language: &str) -> Option<String> {
    let data = L10N.get()?;
    let lang_code = normalize_language_code(language);

    for lang in [&lang_code, &data.fallback_language] {
        if let Some(translations) = data.translations.get(lang.as_str()) {
            if let Some(text) = translations.get(translation_key) {
                return Some(text.clone());
            }
        }
    }
    None
}

/// l10n 模块是否已初始化。
pub fn is_initialized() -> bool {
    L10N.get().is_some()
}

/// 配置的回退语言。
pub fn fallback_language() -> Option<&'static str> {
    L10N.get().map(|data| data.fallback_language.as_str())
}

/// 可用语言代码列表。
pub fn available_languages() -> Vec<String> {
    L10N.get()
        .map(|data| data.translations.keys().cloned().collect())
        .unwrap_or_default()
}

fn registry_key_to_translation_key(registry_key: &str, prefix: &str) -> String {
    if let Some((namespace, path)) = registry_key.split_once(':') {
        format!("{prefix}.{namespace}.{path}")
    } else {
        format!("{prefix}.minecraft.{registry_key}")
    }
}

fn normalize_language_code(language: &str) -> String {
    // "zh-CN" / "zh_CN" -> "zh_cn"（Mojang 语言文件用小写下划线）
    language.to_lowercase().replace('-', "_")
}

/// 为给定语言代码查找语言文件路径：先精确匹配，再前缀匹配
/// （如 "zh" 匹配 "zh_cn.json"）。
pub fn find_lang_file(lang_dir: &Path, language: &str) -> Option<PathBuf> {
    let normalized = normalize_language_code(language);
    let exact = lang_dir.join(format!("{normalized}.json"));
    if exact.exists() {
        return Some(exact);
    }

    // 前缀匹配（如 "zh" 匹配 "zh_cn"、"zh_hk"、"zh_tw"）
    let prefix = normalized.split('_').next().unwrap_or(&normalized);

    if let Ok(dir) = std::fs::read_dir(lang_dir) {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
                && (name.eq_ignore_ascii_case(&normalized)
                    || name.to_lowercase().starts_with(prefix))
            {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_language_code() {
        assert_eq!(normalize_language_code("zh_CN"), "zh_cn");
        assert_eq!(normalize_language_code("zh-CN"), "zh_cn");
        assert_eq!(normalize_language_code("en_US"), "en_us");
        assert_eq!(normalize_language_code("EN_us"), "en_us");
    }

    #[test]
    fn test_registry_key_to_translation_key() {
        assert_eq!(
            registry_key_to_translation_key("minecraft:stone", "block"),
            "block.minecraft.stone"
        );
        assert_eq!(
            registry_key_to_translation_key("minecraft:diamond_sword", "item"),
            "item.minecraft.diamond_sword"
        );
        assert_eq!(
            registry_key_to_translation_key("stone", "block"),
            "block.minecraft.stone"
        );
    }
}
