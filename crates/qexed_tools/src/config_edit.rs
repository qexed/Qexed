use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml_edit::DocumentMut;

pub fn read_server_doc(config: &Path) -> Result<(PathBuf, DocumentMut)> {
    let path = server_config_path(config);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("无法读取配置文件 {}", path.display()))?;
    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("配置文件不是合法 TOML: {}", path.display()))?;
    Ok((path, doc))
}

pub fn write_doc(path: &Path, doc: &DocumentMut) -> Result<()> {
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("无法写入配置文件 {}", path.display()))
}

fn server_config_path(config: &Path) -> PathBuf {
    let split_path = split_dir_for(config).join("server.toml");
    if split_path.exists() {
        split_path
    } else {
        config.to_path_buf()
    }
}

fn split_dir_for(config: &Path) -> PathBuf {
    let stem = config
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("qexed");
    config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.d"))
}
