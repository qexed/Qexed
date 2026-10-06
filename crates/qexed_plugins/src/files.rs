//! 插件目录扫描（v4 plugins/files.rs 迁移；v6 加载 dll/so 动态库而非 wasm）。

use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) const PLUGIN_DIR: &str = "plugins";

/// 动态库扩展名（Windows: dll，Unix: so；两者都接受以便交叉部署目录）。
fn is_library_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dll") || extension == "so")
}

pub(crate) fn plugin_files(path: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && is_library_file(path))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    files.sort();
    files
}
