pub mod config;
pub mod error;
mod download;
mod extract;
mod jdk;
mod lock;
mod manifest;
mod paths;
mod reports;

use std::path::Path;
use std::time::Duration;

use qexed_config::Config;

use self::{
    config::MojangDataConfig,
    download::download_if_needed,
    extract::extract_minecraft_data,
    lock::DownloadLock,
    manifest::{resolve_assets, resolve_java_major},
    paths::{data_root_ready, mojang_cache_root},
};
use crate::error::{IoCtx, MojangDataError};

/// 覆盖 Mojang 缓存根目录（测试/多实例部署；仅首次调用生效）。
pub use paths::set_cache_dir;

/// 本 crate 锁定的 Minecraft 版本（下载与缓存目录命名用）。
pub const MC_VERSION: &str = "26.1.2";
/// 本 crate 锁定的 Minecraft 协议版本号。
pub const PROTOCOL_VERSION: i32 = 775;

/// 加载配置、下载 Mojang jar（如需）并解压 `data/minecraft` 到缓存目录；
/// 再按需运行数据生成器产出静态注册表报告（reports/registries.json 等）。
pub async fn init() -> Result<(), MojangDataError> {
    let config = MojangDataConfig::load_and_create_default(true)?;
    tokio::task::spawn_blocking(move || sync_data(&config))
        .await
        .map_err(|err| MojangDataError::Join(err.to_string()))??;
    Ok(())
}

/// 静态注册表报告目录（cache/mojang/<version>/reports）。
/// 报告由 [`init`] 自动生成，消费方通过本函数取路径。
pub fn reports_dir() -> Result<std::path::PathBuf, MojangDataError> {
    let dir = mojang_cache_root().join(MC_VERSION).join("reports");
    if reports::reports_ready_for_api(&dir) {
        Ok(dir)
    } else {
        Err(MojangDataError::ReportsNotReady(dir))
    }
}

/// 动态注册表数据目录（cache/mojang/<version>/data/minecraft）。
/// 包含各注册表的 JSON 内容与 tags 目录，供 registry 同步使用。
pub fn data_dir() -> Result<std::path::PathBuf, MojangDataError> {
    let dir = mojang_cache_root().join(MC_VERSION).join("data/minecraft");
    if dir.join("dimension_type").is_dir() {
        Ok(dir)
    } else {
        Err(MojangDataError::DataNotReady(dir))
    }
}

fn sync_data(config: &MojangDataConfig) -> Result<std::path::PathBuf, MojangDataError> {
    let version = MC_VERSION;
    let cache_root = mojang_cache_root().join(version);
    let data_root = cache_root.join("data/minecraft");

    let need_data = !data_root_ready(&data_root);
    let need_reports = config.datagen && !reports::reports_ready_for_api(&cache_root.join("reports"));

    if !need_data && !need_reports {
        return Ok(data_root);
    }

    let lock_path = cache_root.join(".download.lock");
    let _lock = DownloadLock::acquire(&lock_path)?;
    let need_data = !data_root_ready(&data_root);
    let need_reports =
        config.datagen && !reports::reports_ready_for_api(&cache_root.join("reports"));
    if !need_data && !need_reports {
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

    // ---- 阶段 1：data/minecraft（沿袭原逻辑：server 优先，逐个尝试） ----
    let mut server_jar: Option<std::path::PathBuf> = None;
    if need_data {
        let assets = resolve_assets(&client, &config.version_manifest_url, version)?;
        let mut errors = Vec::new();

        for (kind, asset) in assets {
            let jar_path = cache_root.join(format!("{kind}.jar"));
            download_if_needed(&client, kind, &asset, &jar_path)?;
            match extract_minecraft_data(&jar_path, &cache_root) {
                Ok(()) => {
                    if !data_root_ready(&data_root) {
                        return Err(MojangDataError::DataNotReady(data_root));
                    }
                    // datagen 需要 server.jar；extract 成功后暂不删除。
                    if kind == "server" {
                        server_jar = Some(jar_path);
                    }
                    break;
                }
                Err(err) => {
                    errors.push(format!("{kind}: {err}"));
                }
            }
        }

        if !data_root_ready(&data_root) {
            return Err(MojangDataError::ExtractFailed(errors.join("; ")));
        }
    }

    // ---- 阶段 2：reports（datagen --reports） ----
    if need_reports {
        // server.jar 可能已被旧版本流程删除，缺失时重新解析下载。
        let jar_path = match server_jar {
            Some(path) => path,
            None => {
                let jar = cache_root.join("server.jar");
                if jar.is_file() {
                    jar
                } else {
                    let assets =
                        resolve_assets(&client, &config.version_manifest_url, version)?;
                    let server = assets
                        .into_iter()
                        .find(|(kind, _)| *kind == "server")
                        .map(|(_, asset)| asset)
                        .ok_or_else(|| MojangDataError::MissingDownloads(version.to_string()))?;
                    download_if_needed(&client, "server", &server, &jar)?;
                    jar
                }
            }
        };

        // Java 就绪（发现 → 校验 → 下载 Temurin）
        let java_major = resolve_java_major(
            &client,
            &config.version_manifest_url,
            version,
        )?;
        let java = jdk::ensure_java(
            &config.java_path,
            config.jdk_download,
            java_major,
            &cache_root,
        )?;

        reports::ensure_reports(&java.java_bin, &jar_path, &cache_root, version)?;
        cleanup_downloaded_jars(&cache_root);
    } else if server_jar.is_none() {
        // 无需 reports 且这次没新下 jar：维持"解压完即删"旧行为。
        cleanup_downloaded_jars(&cache_root);
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
