use std::path::PathBuf;

use crate::error::ConfigError;

#[cfg(feature = "global-root")]
static CONFIG_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// 初始化进程级全局配置根目录（需要 `global-root` 特征）。
///
/// 一个进程只能调用一次；之后所有未覆写 `Config::ROOT` 的实现共享该根目录。
#[cfg(feature = "global-root")]
pub fn init_config_path(path: PathBuf) -> Result<(), ConfigError> {
    CONFIG_PATH
        .set(path)
        .map_err(|_| ConfigError::AlreadyInitialized)
}

/// 读取进程级全局配置根目录（需要 `global-root` 特征）。
#[cfg(feature = "global-root")]
pub fn config_path() -> Result<&'static PathBuf, ConfigError> {
    CONFIG_PATH.get().ok_or(ConfigError::NotInitialized)
}

/// 全局根目录下推导配置文件路径（需要 `global-root` 特征）。
#[cfg(feature = "global-root")]
pub fn config_file(path: &str, name: &str) -> Result<PathBuf, ConfigError> {
    config_file_at(config_path()?, path, name)
}

/// 在指定根目录下推导配置文件路径（不依赖全局根目录）。
///
/// 供 `Config::ROOT` 覆写与第三方动态根目录使用。
pub fn config_file_at(
    base: &std::path::Path,
    path: &str,
    name: &str,
) -> Result<PathBuf, ConfigError> {
    validate_config_name(name)?;
    let dir = build_safe_path(base, path)?;
    Ok(dir.join(format!("{name}.toml")))
}

/// 文档目录下推导文档文件路径（与全局根目录无关，恒可用）。
pub fn config_doc_path(path: &str, name: &str) -> Result<PathBuf, ConfigError> {
    validate_config_name(name)?;
    let dir = build_safe_relative(path)?;
    Ok(dir.join(name))
}

fn build_safe_relative(path: &str) -> Result<PathBuf, ConfigError> {
    let sub_trimmed = path.trim_start_matches('/');
    let sub_path = std::path::Path::new(sub_trimmed);

    for comp in sub_path.components() {
        if matches!(
            comp,
            std::path::Component::ParentDir | std::path::Component::CurDir
        ) {
            return Err(ConfigError::InvalidPath(format!(
                "sub path contains illegal component: {comp:?}"
            )));
        }
    }

    Ok(sub_path.to_path_buf())
}
fn validate_config_name(name: &str) -> Result<(), ConfigError> {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Ok(());
    }
    Err(ConfigError::InvalidName(name.to_string()))
}

fn build_safe_path(base: &std::path::Path, path: &str) -> Result<PathBuf, ConfigError> {
    let base_abs = std::path::absolute(base)?; // io::Error → Io 变体

    let sub_trimmed = path.trim_start_matches('/');
    let sub_path = std::path::Path::new(sub_trimmed);

    for comp in sub_path.components() {
        if matches!(
            comp,
            std::path::Component::ParentDir | std::path::Component::CurDir
        ) {
            return Err(ConfigError::InvalidPath(format!(
                "sub path contains illegal component: {comp:?}"
            )));
        }
    }

    let final_path = base_abs.join(sub_path);
    if !final_path.starts_with(&base_abs) {
        return Err(ConfigError::InvalidPath(
            "final path escapes base directory".into(),
        ));
    }

    Ok(final_path)
}