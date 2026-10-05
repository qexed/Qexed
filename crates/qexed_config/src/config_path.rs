use std::path::PathBuf;



use crate::error::ConfigError;
// 必须初始化！！！
static CONFIG_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
/// 初始化阶段调用
pub fn init_config_path(path: PathBuf) -> Result<(), ConfigError> {
    CONFIG_PATH
        .set(path)
        .map_err(|_| ConfigError::AlreadyInitialized)
}
pub fn config_path() -> Result<&'static PathBuf, ConfigError> {
    CONFIG_PATH.get().ok_or(ConfigError::NotInitialized)
}
pub fn config_file(path: &str, name: &str) -> Result<PathBuf, ConfigError> {
    validate_config_name(name)?;
    let base = config_path()?;
    let dir = build_safe_path(base, path)?;
    Ok(dir.join(format!("{name}.toml")))
}

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
