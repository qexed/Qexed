use std::path::{Path, PathBuf};

pub(super) const CACHE_DIR: &str = "cache/mojang";
pub(super) const DATA_MARKER: &str = ".qexed-data-ready";

pub(super) fn mojang_cache_root() -> PathBuf {
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
