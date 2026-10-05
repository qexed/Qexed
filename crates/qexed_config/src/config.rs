//! `Config` trait：定义配置文件的位置、名称、机密字段规则，
//! 并提供 create / load / save 三个模板方法。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{config_path::config_file, error::ConfigError};

/// 确保路径的父目录存在（相当于 `mkdir -p $(dirname path)`）。
fn ensure_parent(path: &std::path::Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(ConfigError::from)?;
        }
    }
    Ok(())
}

/// 机密文件路径：与主文件同模块目录，位于 `<PATH>/.secrets/<NAME>.toml`。
fn secret_file(path: &str, name: &str) -> Result<PathBuf, ConfigError> {
    let rel = format!("{path}/.secrets");
    config_file(&rel, name)
}

pub trait Config: Serialize + for<'de> Deserialize<'de> + Default + Sized {
    /// 文件所在目录路径(相对配置文件夹)
    const PATH: &'static str;
    /// 文件名(相对文件所在目录路径)（不用写.toml,会自动补全的)
    const NAME: &'static str;

    /// 机密字段
    ///
    /// 保存时会把这些字段从主配置文件抽到 `<PATH>/.secrets/<name>.toml`，
    /// 主文件对应位置写占位符 `<stored in .secrets>`；
    /// 加载时再从 secrets 文件回填真实值。
    ///
    /// 例如：
    ///
    /// ```rust
    /// const SECRETS: &'static [&'static str] = &[
    ///     "token",                 // 顶层字段
    ///     "auth.password",         // 嵌套对象
    ///     "servers.*.api_key",     // 数组每项都处理
    /// ];
    /// ```
    ///
    /// 对应的主配置文件（`config.toml`）：
    ///
    /// ```toml
    /// token = "<stored in .secrets>"
    ///
    /// [auth]
    /// username = "alice"
    /// password = "<stored in .secrets>"
    ///
    /// [[servers]]
    /// host = "a.example.com"
    /// api_key = "<stored in .secrets>"
    /// ```
    ///
    /// 对应的机密文件（`.secrets/config.toml`）：
    ///
    /// ```toml
    /// token = "sk-xxxxxxxxxxxx"
    ///
    /// [auth]
    /// password = "hunter2"
    ///
    /// [[servers]]
    /// api_key = "key-a"
    /// ```
    const SECRETS: &'static [&'static str] = &[];

    // 读取文件(没有则新建)
    // 若save为true,则读取后覆盖原文件
    fn load_and_create_default(save: bool) -> Result<Self, ConfigError> {
        let path = config_file(Self::PATH, Self::NAME)?;
        if !qexed_toml::has_file(&path)? {
            Self::create_file(None)?;
        }
        Ok(Self::load_file(save)?)
    }

    // 新建文件
    // 按 SECRETS 规则拆成主文件 + secrets 文件，分别落盘
    fn create_file(config: Option<&Self>) -> Result<(), ConfigError> {
        let path = config_file(Self::PATH, Self::NAME)?;
        let secret_path = secret_file(Self::PATH, Self::NAME)?;

        let doc = match config {
            Some(c) => qexed_toml::to_document(c)?,
            None => qexed_toml::to_document(&Self::default())?,
        };

        let (main_doc, secret_doc) = qexed_toml::split_secrets(&doc, Self::SECRETS, None)?;

        ensure_parent(&path)?;
        qexed_toml::create_file(&path, &main_doc)?;

        if !secret_doc.as_table().is_empty() {
            ensure_parent(&secret_path)?;
            qexed_toml::create_file(&secret_path, &secret_doc)?;
        }

        Ok(())
    }

    // 读取文件
    // 若save为true,则读取后覆盖原文件
    fn load_file(save: bool) -> Result<Self, ConfigError> {
        let path = config_file(Self::PATH, Self::NAME)?;
        let secret_path = secret_file(Self::PATH, Self::NAME)?;

        let config_documentmut = qexed_toml::load_file(&path)?;

        // 先与默认值合并，补齐新版本新增字段
        let mut config_load = qexed_toml::merge(
            &qexed_toml::to_document(&Self::default())?,
            &config_documentmut,
        )?;

        // 再把 secrets 回填进来（占位符被真实值替换）
        if qexed_toml::has_file(&secret_path)? {
            let secret_documentmut = qexed_toml::load_file(&secret_path)?;
            config_load = qexed_toml::merge_secrets(&config_load, &secret_documentmut)?;
        }

        let config: Self = qexed_toml::from_document(config_load)?;
        if save {
            Self::save_file(&config)?;
        }
        Ok(config)
    }

    // 保存文件
    // 先把新配置合到磁盘上的旧主文件（保留未知字段），
    // 再按 SECRETS 拆成主文件 + secrets 文件落盘
    fn save_file(config: &Self) -> Result<(), ConfigError> {
        let path = config_file(Self::PATH, Self::NAME)?;
        let secret_path = secret_file(Self::PATH, Self::NAME)?;

        let new_doc = qexed_toml::to_document(config)?;

        // 与磁盘上的旧主文件合并，保留我们"不认识"的字段
        let merged_doc = if qexed_toml::has_file(&path)? {
            let old_doc = qexed_toml::load_file(&path)?;
            qexed_toml::merge(&old_doc, &new_doc)?
        } else {
            new_doc
        };

        // 读旧 secret，用于"一旦机密，永远是机密"的规则扩展
        let old_secret = if qexed_toml::has_file(&secret_path)? {
            Some(qexed_toml::load_file(&secret_path)?)
        } else {
            None
        };

        let (main_doc, secret_doc) =
            qexed_toml::split_secrets(&merged_doc, Self::SECRETS, old_secret.as_ref())?;

        ensure_parent(&path)?;
        qexed_toml::save_file(&path, &main_doc)?;

        if !secret_doc.as_table().is_empty() {
            ensure_parent(&secret_path)?;
            qexed_toml::save_file(&secret_path, &secret_doc)?;
        } else if qexed_toml::has_file(&secret_path)? {
            // secrets 全空 → 删掉旧文件，避免下次误加载
            std::fs::remove_file(&secret_path).map_err(ConfigError::from)?;
        }

        Ok(())
    }

    // 将结构体转换为toml字符串
    fn to_toml_string(config: &Self)->Result<String,ConfigError>{
        Ok(qexed_toml::to_document(config)?.to_string())
    }
}