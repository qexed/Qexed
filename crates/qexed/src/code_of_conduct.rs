use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result};

pub const DEFAULT_CODE_OF_CONDUCT_DIR: &str = "config/enable-code-of-conduct";

#[derive(Debug, Default, Clone)]
pub struct CodeOfConductTexts {
    texts: BTreeMap<String, String>,
}

impl CodeOfConductTexts {
    pub fn load(enabled: bool, dir: impl AsRef<Path>) -> Result<Self> {
        if !enabled {
            return Ok(Self::default());
        }

        Self::load_from_dir(dir)
    }

    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            anyhow::bail!("code-of-conduct folder does not exist: {}", dir.display());
        }

        let real_dir = dir.canonicalize().with_context(|| {
            format!("failed to resolve code-of-conduct folder {}", dir.display())
        })?;
        let mut texts = BTreeMap::new();

        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("failed to read code-of-conduct folder {}", dir.display()))?
        {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read an entry in code-of-conduct folder {}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            if !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
            {
                continue;
            }

            let real_path = path.canonicalize().with_context(|| {
                format!("failed to resolve code-of-conduct file {}", path.display())
            })?;
            let parent = real_path.parent().ok_or_else(|| {
                anyhow::anyhow!("code-of-conduct file has no parent: {}", path.display())
            })?;
            if parent != real_dir {
                anyhow::bail!(
                    "code-of-conduct file {} points outside {}",
                    path.display(),
                    dir.display()
                );
            }

            let language = path
                .file_stem()
                .map(|stem| normalize_language_code(&stem.to_string_lossy()))
                .filter(|language| !language.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "code-of-conduct file has no language name: {}",
                        path.display()
                    )
                })?;
            let text = std::fs::read_to_string(&path).with_context(|| {
                format!("failed to read code-of-conduct file {}", path.display())
            })?;
            texts.insert(language, normalize_newlines(text));
        }

        Ok(Self { texts })
    }

    pub fn select(&self, locale: Option<&str>) -> Option<&str> {
        if self.texts.is_empty() {
            return None;
        }

        locale
            .map(normalize_language_code)
            .and_then(|language| self.texts.get(&language))
            .or_else(|| self.texts.get("en_us"))
            .or_else(|| self.texts.values().next())
            .map(String::as_str)
    }
}

fn normalize_language_code(value: &str) -> String {
    let normalized = value.trim().replace('-', "_").to_ascii_lowercase();
    match normalized.as_str() {
        "" => String::new(),
        "en" => "en_us".to_string(),
        "zh" | "zh_cn" | "zh_hans" => "zh_cn".to_string(),
        other => other.to_string(),
    }
}

fn normalize_newlines(value: String) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_language_files_and_selects_client_locale() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("en_us.txt"), "English rules").unwrap();
        std::fs::write(dir.path().join("zh-CN.txt"), "Chinese rules").unwrap();

        let texts = CodeOfConductTexts::load_from_dir(dir.path()).unwrap();

        assert_eq!(texts.select(Some("zh_cn")), Some("Chinese rules"));
        assert_eq!(texts.select(Some("zh-CN")), Some("Chinese rules"));
        assert_eq!(texts.select(Some("fr_fr")), Some("English rules"));
    }

    #[test]
    fn disabled_loader_ignores_missing_folder() {
        let texts = CodeOfConductTexts::load(false, "missing").unwrap();
        assert!(texts.select(None).is_none());
    }
}
