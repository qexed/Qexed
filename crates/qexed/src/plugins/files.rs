use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) const PLUGIN_DIR: &str = "plugins";

pub(super) fn plugin_files(path: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "wasm")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    files.sort();
    files
}
