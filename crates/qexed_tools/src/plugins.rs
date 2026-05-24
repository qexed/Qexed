use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PluginEntry {
    pub name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub size: u64,
}

pub fn list(dir: impl AsRef<Path>) -> Result<Vec<PluginEntry>> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir).with_context(|| format!("无法创建插件目录 {}", dir.display()))?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("无法读取插件目录 {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !is_plugin_file(&path) {
            continue;
        }
        let metadata = entry.metadata()?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin.wasm")
            .to_string();
        let enabled = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"));
        entries.push(PluginEntry {
            name: file_name,
            path,
            enabled,
            size: metadata.len(),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

pub fn install(dir: impl AsRef<Path>, source: impl AsRef<Path>) -> Result<PathBuf> {
    let dir = dir.as_ref();
    let source = source.as_ref();
    fs::create_dir_all(dir).with_context(|| format!("无法创建插件目录 {}", dir.display()))?;
    ensure_wasm(source)?;

    let file_name = source
        .file_name()
        .context("插件路径缺少文件名")?
        .to_os_string();
    let target = dir.join(file_name);
    fs::copy(source, &target)
        .with_context(|| format!("无法安装插件 {} -> {}", source.display(), target.display()))?;
    Ok(target)
}

pub fn enable(dir: impl AsRef<Path>, name: &str) -> Result<PathBuf> {
    let disabled = plugin_path(dir.as_ref(), name, false);
    let enabled = plugin_path(dir.as_ref(), name, true);
    rename_plugin(&disabled, &enabled)
}

pub fn disable(dir: impl AsRef<Path>, name: &str) -> Result<PathBuf> {
    let enabled = plugin_path(dir.as_ref(), name, true);
    let disabled = plugin_path(dir.as_ref(), name, false);
    rename_plugin(&enabled, &disabled)
}

pub fn remove(dir: impl AsRef<Path>, name: &str) -> Result<()> {
    let enabled = plugin_path(dir.as_ref(), name, true);
    let disabled = plugin_path(dir.as_ref(), name, false);
    if enabled.exists() {
        fs::remove_file(&enabled).with_context(|| format!("无法删除插件 {}", enabled.display()))?;
    }
    if disabled.exists() {
        fs::remove_file(&disabled)
            .with_context(|| format!("无法删除插件 {}", disabled.display()))?;
    }
    Ok(())
}

fn rename_plugin(from: &Path, to: &Path) -> Result<PathBuf> {
    if !from.exists() {
        anyhow::bail!("插件不存在: {}", from.display());
    }
    fs::rename(from, to)
        .with_context(|| format!("无法重命名插件 {} -> {}", from.display(), to.display()))?;
    Ok(to.to_path_buf())
}

fn plugin_path(dir: &Path, name: &str, enabled: bool) -> PathBuf {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);
    if enabled {
        dir.join(format!("{stem}.wasm"))
    } else {
        dir.join(format!("{stem}.wasm.disabled"))
    }
}

fn ensure_wasm(path: &Path) -> Result<()> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"))
    {
        Ok(())
    } else {
        anyhow::bail!("插件必须是 .wasm 文件: {}", path.display())
    }
}

fn is_plugin_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with(".wasm") || name.ends_with(".wasm.disabled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enables_and_disables_plugin_by_suffix() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.wasm"), b"wasm").unwrap();

        disable(dir.path(), "a").unwrap();
        assert!(dir.path().join("a.wasm.disabled").exists());
        enable(dir.path(), "a").unwrap();
        assert!(dir.path().join("a.wasm").exists());
    }
}
