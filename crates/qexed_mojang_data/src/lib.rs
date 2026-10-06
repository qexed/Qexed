pub mod config;
pub mod error;
mod download;
mod extract;
mod lock;
mod manifest;
mod paths;

use std::path::Path;
use std::time::Duration;

use qexed_config::Config;

use self::{
    config::MojangDataConfig,
    download::download_if_needed,
    extract::extract_minecraft_data,
    lock::DownloadLock,
    manifest::resolve_assets,
    paths::{data_root_ready, mojang_cache_root},
};
use crate::error::{IoCtx, MojangDataError};

/// 加载配置、下载 Mojang jar（如需）并解压 `data/minecraft` 到缓存目录。
pub async fn init() -> Result<(), MojangDataError> {
    let config = MojangDataConfig::load_and_create_default(true)?;
    tokio::task::spawn_blocking(move || sync_data(&config))
        .await
        .map_err(|err| MojangDataError::Join(err.to_string()))??;
    Ok(())
}

fn sync_data(config: &MojangDataConfig) -> Result<std::path::PathBuf, MojangDataError> {
    let version = qexed_config::MC_VERSION;
    let cache_root = mojang_cache_root().join(version);
    let data_root = cache_root.join("data/minecraft");

    if data_root_ready(&data_root) {
        return Ok(data_root);
    }

    let lock_path = cache_root.join(".download.lock");
    let _lock = DownloadLock::acquire(&lock_path)?;
    if data_root_ready(&data_root) {
        return Ok(data_root);
    }

    std::fs::create_dir_all(&cache_root).io_ctx(format!(
        "无法创建 Mojang 缓存目录: {}",
        cache_root.display()
    ))?;

    let timeout = Duration::from_secs(u64::from(config.http_timeout.max(1)));
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(format!("qexed/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let assets = resolve_assets(&client, &config.version_manifest_url, version)?;
    let mut errors = Vec::new();

    for (kind, asset) in assets {
        let jar_path = cache_root.join(format!("{kind}.jar"));
        download_if_needed(&client, kind, &asset, &jar_path)?;
        match extract_minecraft_data(&jar_path, &cache_root) {
            Ok(()) => {
                cleanup_downloaded_jars(&cache_root);
                if !data_root_ready(&data_root) {
                    return Err(MojangDataError::DataNotReady(data_root));
                }
                return Ok(data_root);
            }
            Err(err) => {
                errors.push(format!("{kind}: {err}"));
            }
        }
    }

    if !data_root_ready(&data_root) {
        return Err(MojangDataError::ExtractFailed(errors.join("; ")));
    }

    Ok(data_root)
}

fn cleanup_downloaded_jars(cache_root: &Path) {
    for name in ["server.jar", "client.jar"] {
        let path = cache_root.join(name);
        if let Err(err) = std::fs::remove_file(&path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!(
                "failed to remove Mojang jar after data extraction: path={}, error={err}",
                path.display()
            );
        }
    }
}
