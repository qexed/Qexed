use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct WorldgenCache {
    data_root: PathBuf,
    reports_root: PathBuf,
}
impl WorldgenCache {
    pub fn from_roots(data_root: impl Into<PathBuf>, reports_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            reports_root: reports_root.into(),
        }
    }

    pub fn default_mojang_cache() -> Self {
        let cache_root = workspace_root()
            .join("cache/mojang")
            .join(qexed_config::MC_VERSION);
        Self::from_roots(
            cache_root.join("data/minecraft"),
            cache_root.join("generated/reports"),
        )
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn reports_root(&self) -> &Path {
        &self.reports_root
    }

    pub fn ensure_ready(&self) -> Result<()> {
        if !self.data_root.join("worldgen").is_dir() {
            anyhow::bail!(
                "Mojang worldgen cache is unavailable: {}",
                self.data_root.display()
            );
        }
        if !self.reports_root.join("blocks.json").is_file()
            || !self.reports_root.join("registries.json").is_file()
        {
            anyhow::bail!(
                "Mojang reports cache is unavailable: {}",
                self.reports_root.display()
            );
        }
        Ok(())
    }
}

impl Default for WorldgenCache {
    fn default() -> Self {
        Self::default_mojang_cache()
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
