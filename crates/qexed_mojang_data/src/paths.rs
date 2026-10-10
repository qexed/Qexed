use std::path::{Path, PathBuf};

pub(super) const CACHE_DIR: &str = "cache/mojang";
pub(super) const DATA_MARKER: &str = ".qexed-data-ready";

/// 进程级缓存根覆盖（测试/嵌入式场景），优先于 env 与默认路径。
static CACHE_DIR_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// 覆盖 Mojang 缓存根目录（仅首次调用生效）。测试与多实例部署用。
pub fn set_cache_dir(path: impl Into<PathBuf>) -> bool {
    CACHE_DIR_OVERRIDE.set(path.into()).is_ok()
}

pub(super) fn mojang_cache_root() -> PathBuf {
    if let Some(path) = CACHE_DIR_OVERRIDE.get() {
        return path.clone();
    }
    if let Ok(path) = std::env::var("QEXED_MOJANG_CACHE_DIR")
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }
    resolve_cache_path(PathBuf::from(CACHE_DIR))
}

fn resolve_cache_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

pub(super) fn data_root_ready(data_root: &Path) -> bool {
    data_root.join(DATA_MARKER).is_file()
        && data_root.join("dimension_type").is_dir()
        && data_root.join("damage_type").is_dir()
        && data_root.join("tags/damage_type").is_dir()
}
