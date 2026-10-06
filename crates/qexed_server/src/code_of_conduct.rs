//! 行为准则（code of conduct）多语言文本加载。
//! 迁移自 v4 crates/qexed/src/code_of_conduct.rs；anyhow → crate::error::ServerError。
//!
//! v4 测试用 tempfile crate；v6 服务器 crate 不引该依赖，测试改用
//! std::env::temp_dir + 唯一子目录（手动清理）。

use std::{collections::BTreeMap, path::Path};

use crate::error::{Result, ServerError};

pub const DEFAULT_CODE_OF_CONDUCT_DIR: &str = "config/enable-code-of-conduct";

#[derive(Debug, Default, Clone)]
pub struct CodeOfConductTexts {
    texts: BTreeMap<String, String>,
}

impl CodeOfConductTexts {
    /// 按配置加载：未启用时返回空表（目录缺失不算错误）。
    pub fn load(enabled: bool, dir: impl AsRef<Path>) -> Result<Self> {
        if !enabled {
            return Ok(Self::default());
        }

        Self::load_from_dir(dir)
    }

    /// 从目录加载 <language>.txt 文本文件；目录缺失或文件逃逸时报错。
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Err(ServerError::msg(format!(
                "code-of-conduct folder does not exist: {}",
                dir.display()
            )));
        }

        let real_dir = dir.canonicalize().map_err(|source| ServerError::IoContext {
            context: format!("failed to resolve code-of-conduct folder {}", dir.display()),
            source,
        })?;
        let mut texts = BTreeMap::new();

        let entries = std::fs::read_dir(dir).map_err(|source| ServerError::IoContext {
            context: format!("failed to read code-of-conduct folder {}", dir.display()),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| ServerError::IoContext {
                context: format!(
                    "failed to read an entry in code-of-conduct folder {}",
                    dir.display()
                ),
                source,
            })?;
            let path = entry.path();
            if !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
            {
                continue;
            }

            let real_path = path.canonicalize().map_err(|source| ServerError::IoContext {
                context: format!("failed to resolve code-of-conduct file {}", path.display()),
                source,
            })?;
            let parent = real_path.parent().ok_or_else(|| {
                ServerError::msg(format!("code-of-conduct file has no parent: {}", path.display()))
            })?;
            if parent != real_dir {
                return Err(ServerError::msg(format!(
                    "code-of-conduct file {} points outside {}",
                    path.display(),
                    dir.display()
                )));
            }

            let language = path
                .file_stem()
                .map(|stem| normalize_language_code(&stem.to_string_lossy()))
                .filter(|language| !language.is_empty())
                .ok_or_else(|| {
                    ServerError::msg(format!(
                        "code-of-conduct file has no language name: {}",
                        path.display()
                    ))
                })?;
            let text = std::fs::read_to_string(&path).map_err(|source| ServerError::IoContext {
                context: format!("failed to read code-of-conduct file {}", path.display()),
                source,
            })?;
            texts.insert(language, normalize_newlines(text));
        }

        Ok(Self { texts })
    }

    /// 按客户端 locale 选择文本：请求语言 → en_us → 任意第一个。
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

/// 归一化语言代码：zh/zh_hans → zh_cn，en → en_us，其余小写下划线。
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
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 简易临时目录（v6 不引 tempfile）：temp_dir 下唯一子目录，测试末尾手动删除。
    fn tempdir(name: &str) -> std::path::PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("qexed_server_coc_{name}_{seq}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn loads_language_files_and_selects_client_locale() {
        let dir = tempdir("load");
        std::fs::write(dir.join("en_us.txt"), "English rules").unwrap();
        std::fs::write(dir.join("zh-CN.txt"), "Chinese rules").unwrap();

        let texts = CodeOfConductTexts::load_from_dir(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

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
