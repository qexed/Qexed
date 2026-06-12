use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use serde_json::Value;

static L10N: OnceLock<L10nData> = OnceLock::new();

#[derive(Debug, Clone)]
struct L10nData {
    translations: HashMap<String, HashMap<String, String>>,
    fallback_language: String,
}

/// Initialize the l10n module with the path to the lang directory.
/// The lang directory should contain Minecraft language JSON files
/// (e.g., en_us.json, zh_cn.json).
pub fn initialize(lang_dir: &Path, fallback_language: impl Into<String>) -> Result<()> {
    let fallback = fallback_language.into();
    let mut translations = HashMap::new();

    if !lang_dir.exists() {
        log::warn!(
            "l10n lang directory does not exist: {}, i18n will be unavailable",
            lang_dir.display()
        );
        return L10N
            .set(L10nData {
                translations,
                fallback_language: fallback,
            })
            .map_err(|_| anyhow::anyhow!("l10n already initialized"));
    }

    for entry in std::fs::read_dir(lang_dir)
        .with_context(|| format!("无法读取 lang 目录: {}", lang_dir.display()))?
    {
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

        // Load the language JSON file
        match load_lang_file(&path) {
            Ok(entries) => {
                let count = entries.len();
                translations.insert(file_name.to_string(), entries);
                log::debug!("l10n: loaded language '{file_name}' with {count} translations");
            }
            Err(err) => {
                log::warn!(
                    "l10n: failed to load language file {}: {err:#}",
                    path.display()
                );
            }
        }
    }

    if translations.is_empty() {
        log::warn!("l10n: no language files were loaded, i18n will be unavailable");
    } else {
        log::info!(
            "l10n: initialized with {} language(s), fallback={}",
            translations.len(),
            fallback
        );
    }

    L10N
        .set(L10nData {
            translations,
            fallback_language: fallback,
        })
        .map_err(|_| anyhow::anyhow!("l10n already initialized"))
}

fn load_lang_file(path: &Path) -> Result<HashMap<String, String>> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("无法读取语言文件: {}", path.display()))?;
    let value: Value =
        serde_json::from_str(&content).with_context(|| format!("语言文件 JSON 解析失败: {}", path.display()))?;

    let obj = value
        .as_object()
        .with_context(|| format!("语言文件根节点不是对象: {}", path.display()))?;

    let mut entries = HashMap::with_capacity(obj.len());
    for (key, val) in obj {
        if let Some(text) = val.as_str() {
            entries.insert(key.clone(), text.to_string());
        }
    }
    Ok(entries)
}

/// Get the localized display name for an item or block by its registry key
/// (e.g., "minecraft:stone").
///
/// The language code should be a BCP 47-like string from the player's client
/// (e.g., "zh_CN", "en_US"). Returns `None` if no translation is available.
pub fn localize(key: &str, language: &str) -> Option<String> {
    let data = L10N.get()?;
    let lang_code = normalize_language_code(language);

    // Try the requested language first, then fallback
    for lang in [&lang_code, &data.fallback_language] {
        if let Some(translations) = data.translations.get(lang.as_str()) {
            // Try translation key formats: block.<namespace>.<path>, item.<namespace>.<path>
            for prefix in ["block", "item"] {
                let translation_key = registry_key_to_translation_key(key, prefix);
                if let Some(text) = translations.get(&translation_key) {
                    return Some(text.clone());
                }
            }

            // Also try the key directly as-is
            if let Some(text) = translations.get(key) {
                return Some(text.clone());
            }
        }
    }
    None
}

/// Get the localized display name for an entity type by its registry key
/// (e.g., "minecraft:creeper").
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

/// Get the localized display name for an enchantment by its registry key
/// (e.g., "minecraft:sharpness").
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

/// Get the raw translation for a specific translation key.
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

/// Check if the l10n module has been initialized.
pub fn is_initialized() -> bool {
    L10N.get().is_some()
}

/// Get the configured fallback language.
pub fn fallback_language() -> Option<&'static str> {
    L10N.get().map(|data| data.fallback_language.as_str())
}

/// Get the list of available language codes.
pub fn available_languages() -> Vec<String> {
    L10N
        .get()
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
    // Convert "zh-CN" or "zh_CN" -> "zh_cn"
    // Mojang language files use lowercase with underscores
    language.to_lowercase().replace('-', "_")
}

/// Try to find a language file path for the given language code.
/// First tries an exact match, then falls back to prefix match
/// (e.g., "zh_CN" matches "zh_cn.json").
pub fn find_lang_file(lang_dir: &Path, language: &str) -> Option<PathBuf> {
    let normalized = normalize_language_code(language);
    let exact = lang_dir.join(format!("{normalized}.json"));
    if exact.exists() {
        return Some(exact);
    }

    // Try prefix match (e.g., "zh" matches "zh_cn", "zh_hk", "zh_tw")
    let prefix = normalized.split('_').next().unwrap_or(&normalized);

    if let Ok(dir) = std::fs::read_dir(lang_dir) {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if name.eq_ignore_ascii_case(&normalized)
                        || name.to_lowercase().starts_with(prefix)
                    {
                        return Some(path);
                    }
                }
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
