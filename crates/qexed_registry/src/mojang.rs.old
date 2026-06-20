use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};

use super::util::workspace_root;

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const CACHE_DIR: &str = "cache/mojang";
const DATA_MARKER: &str = ".qexed-data-ready";
const HTTP_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) fn data_root() -> Result<PathBuf> {
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

    std::fs::create_dir_all(&cache_root)
        .with_context(|| format!("无法创建 Mojang 缓存目录: {}", cache_root.display()))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(format!("qexed/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("无法创建 Mojang 下载客户端")?;
    let assets = resolve_assets(&client, version)?;
    let mut errors = Vec::new();
    for (kind, asset) in assets {
        let jar_path = cache_root.join(format!("{kind}.jar"));
        download_if_needed(&client, &asset, &jar_path)?;
        match extract_minecraft_data(&jar_path, &cache_root) {
            Ok(()) => {
                cleanup_downloaded_jars(&cache_root);
                if !data_root_ready(&data_root) {
                    anyhow::bail!(
                        "Mojang 数据缓存初始化后仍缺少 data/minecraft: {}",
                        data_root.display()
                    );
                }

                return Ok(data_root);
            }
            Err(err) => {
                errors.push(format!("{kind}: {err:#}"));
                continue;
            }
        }
    }
    if !data_root_ready(&data_root) {
        anyhow::bail!(
            "Mojang jar 中无法提取 registry data，已尝试: {}",
            errors.join("; ")
        );
    }

    Ok(data_root)
}

fn mojang_cache_root() -> PathBuf {
    if let Ok(path) = std::env::var("QEXED_MOJANG_CACHE_DIR")
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }
    if let Some(path) = super::configured_mojang_cache_path() {
        return resolve_cache_path(path);
    }

    resolve_cache_path(PathBuf::from(CACHE_DIR))
}

fn resolve_cache_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    std::env::current_dir()
        .unwrap_or_else(|_| workspace_root())
        .join(path)
}

fn data_root_ready(data_root: &Path) -> bool {
    data_root.join(DATA_MARKER).is_file()
        && data_root.join("dimension_type").is_dir()
        && data_root.join("damage_type").is_dir()
        && data_root.join("tags/damage_type").is_dir()
}

fn resolve_assets(
    client: &reqwest::blocking::Client,
    version: &str,
) -> Result<Vec<(&'static str, DownloadInfo)>> {
    let manifest = get_json::<VersionManifest>(client, VERSION_MANIFEST_URL)?;
    let version_entry = manifest
        .versions
        .into_iter()
        .find(|entry| entry.id == version)
        .with_context(|| format!("Mojang version manifest 中找不到版本: {version}"))?;
    let version_json = get_json::<VersionJson>(client, &version_entry.url)
        .with_context(|| format!("无法读取 Mojang 版本元数据: {}", version_entry.url))?;
    let mut assets = Vec::new();
    if let Some(server) = version_json.downloads.server {
        assets.push(("server", server));
    }
    if let Some(client) = version_json.downloads.client {
        assets.push(("client", client));
    }
    if assets.is_empty() {
        anyhow::bail!("Mojang 版本元数据缺少 server/client 下载项: {version}");
    }
    Ok(assets)
}

fn get_json<T>(client: &reqwest::blocking::Client, url: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let response = client
        .get(url)
        .send()
        .with_context(|| format!("Mojang 请求失败: {url}"))?
        .error_for_status()
        .with_context(|| format!("Mojang 返回非成功状态: {url}"))?;
    response
        .json::<T>()
        .with_context(|| format!("Mojang JSON 解析失败: {url}"))
}

fn download_if_needed(
    client: &reqwest::blocking::Client,
    asset: &DownloadInfo,
    target: &Path,
) -> Result<()> {
    if target.is_file() {
        match verify_sha1(target, &asset.sha1) {
            Ok(()) => return Ok(()),
            Err(err) => {
                log::warn!(
                    "Mojang 缓存 jar 校验失败，将重新下载: path={}, error={err:#}",
                    target.display()
                );
            }
        }
    }

    let response = client
        .get(&asset.url)
        .send()
        .with_context(|| format!("Mojang jar 下载失败: {}", asset.url))?
        .error_for_status()
        .with_context(|| format!("Mojang jar 返回非成功状态: {}", asset.url))?;
    let bytes = response.bytes().context("无法读取 Mojang jar 响应体")?;
    if asset.size > 0 && bytes.len() as u64 != asset.size {
        anyhow::bail!(
            "Mojang jar 大小不匹配: expected={}, actual={}",
            asset.size,
            bytes.len()
        );
    }
    verify_bytes_sha1(&bytes, &asset.sha1)?;

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建 Mojang jar 缓存目录: {}", parent.display()))?;
    }
    let tmp = target.with_extension("jar.tmp");
    std::fs::write(&tmp, &bytes)
        .with_context(|| format!("无法写入 Mojang jar 临时缓存: {}", tmp.display()))?;
    std::fs::rename(&tmp, target)
        .with_context(|| format!("无法保存 Mojang jar 缓存: {}", target.display()))?;
    Ok(())
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

fn verify_sha1(path: &Path, expected: &str) -> Result<()> {
    let mut file = File::open(path)
        .with_context(|| format!("无法打开文件用于 SHA1 校验: {}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("无法读取文件用于 SHA1 校验: {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    verify_digest(hasher.finalize().as_slice(), expected)
}

fn verify_bytes_sha1(bytes: &[u8], expected: &str) -> Result<()> {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    verify_digest(hasher.finalize().as_slice(), expected)
}

fn verify_digest(actual: &[u8], expected: &str) -> Result<()> {
    let actual = hex::encode(actual);
    if !actual.eq_ignore_ascii_case(expected) {
        anyhow::bail!("SHA1 不匹配: expected={expected}, actual={actual}");
    }
    Ok(())
}

fn extract_minecraft_data(jar_path: &Path, cache_root: &Path) -> Result<()> {
    let data_root = cache_root.join("data/minecraft");
    let tmp_root = cache_root.join("data.tmp");
    if tmp_root.exists() {
        std::fs::remove_dir_all(&tmp_root)
            .with_context(|| format!("无法清理 Mojang 临时数据目录: {}", tmp_root.display()))?;
    }
    std::fs::create_dir_all(&tmp_root)
        .with_context(|| format!("无法创建 Mojang 临时数据目录: {}", tmp_root.display()))?;

    let file = File::open(jar_path)
        .with_context(|| format!("无法打开 Mojang jar: {}", jar_path.display()))?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .with_context(|| format!("无法读取 Mojang jar zip: {}", jar_path.display()))?;
    let extracted = extract_data_from_archive(&mut archive, &tmp_root)?;
    let lang_extracted = extract_lang_files_from_archive(&mut archive, &tmp_root)?;

    if extracted == 0 {
        anyhow::bail!("Mojang jar 中没有找到 data/minecraft JSON 数据");
    }

    let total_extracted = extracted + lang_extracted;
    std::fs::write(
        tmp_root.join("minecraft").join(DATA_MARKER),
        total_extracted.to_string(),
    )
    .with_context(|| "无法写入 Mojang 数据缓存完成标记")?;
    if data_root.exists() {
        std::fs::remove_dir_all(&data_root)
            .with_context(|| format!("无法替换旧 Mojang 数据缓存: {}", data_root.display()))?;
    }
    let final_root = cache_root.join("data");
    if final_root.exists() {
        std::fs::remove_dir_all(&final_root)
            .with_context(|| format!("无法清理旧 Mojang 数据目录: {}", final_root.display()))?;
    }
    std::fs::rename(&tmp_root, &final_root)
        .with_context(|| format!("无法安装 Mojang 数据缓存: {}", final_root.display()))?;
    log::info!(
        "Mojang registry data cached: version={}, registry_files={}, lang_files={}, path={}",
        qexed_config::MC_VERSION,
        extracted,
        lang_extracted,
        data_root.display()
    );
    Ok(())
}

fn extract_data_from_archive<R>(archive: &mut zip::ZipArchive<R>, tmp_root: &Path) -> Result<usize>
where
    R: Read + Seek,
{
    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(relative) = strip_minecraft_data_prefix(&name) else {
            continue;
        };
        if relative.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let target = tmp_root.join("minecraft").join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建 Mojang 数据目录: {}", parent.display()))?;
        }
        let mut output = File::create(&target)
            .with_context(|| format!("无法写入 Mojang 数据文件: {}", target.display()))?;
        std::io::copy(&mut entry, &mut output)
            .with_context(|| format!("无法抽取 Mojang 数据文件: {}", target.display()))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn extract_lang_files_from_archive<R>(
    archive: &mut zip::ZipArchive<R>,
    tmp_root: &Path,
) -> Result<usize>
where
    R: Read + Seek,
{
    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(relative) = strip_minecraft_lang_prefix(&name) else {
            continue;
        };
        if relative.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let target = tmp_root.join("minecraft").join("lang").join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建 Mojang lang 目录: {}", parent.display()))?;
        }
        let mut output = File::create(&target)
            .with_context(|| format!("无法写入 Mojang lang 文件: {}", target.display()))?;
        std::io::copy(&mut entry, &mut output)
            .with_context(|| format!("无法抽取 Mojang lang 文件: {}", target.display()))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn strip_minecraft_lang_prefix(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match (components.next(), components.next(), components.next()) {
        (
            Some(std::path::Component::Normal(assets)),
            Some(std::path::Component::Normal(minecraft)),
            Some(std::path::Component::Normal(lang)),
        ) if assets == "assets" && minecraft == "minecraft" && lang == "lang" => {
            Some(components.as_path().to_path_buf())
        }
        _ => None,
    }
}

fn strip_minecraft_data_prefix(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match (components.next(), components.next()) {
        (
            Some(std::path::Component::Normal(data)),
            Some(std::path::Component::Normal(minecraft)),
        ) if data == "data" && minecraft == "minecraft" => Some(components.as_path().to_path_buf()),
        _ => None,
    }
}

struct DownloadLock {
    path: PathBuf,
}

impl DownloadLock {
    fn acquire(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建 Mojang 下载锁目录: {}", parent.display()))?;
        }

        for _ in 0..600 {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(_) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("无法创建 Mojang 下载锁: {}", path.display()));
                }
            }
        }

        anyhow::bail!("等待 Mojang 下载锁超时: {}", path.display());
    }
}

impl Drop for DownloadLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct VersionManifestEntry {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct VersionJson {
    downloads: VersionDownloads,
}

#[derive(Debug, Deserialize)]
struct VersionDownloads {
    server: Option<DownloadInfo>,
    client: Option<DownloadInfo>,
}

#[derive(Debug, Deserialize)]
struct DownloadInfo {
    sha1: String,
    size: u64,
    url: String,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{strip_minecraft_data_prefix, strip_minecraft_lang_prefix};

    #[test]
    fn strips_minecraft_data_prefix() {
        let path = Path::new("data/minecraft/tags/damage_type/is_fire.json");

        assert_eq!(
            strip_minecraft_data_prefix(path).unwrap(),
            Path::new("tags/damage_type/is_fire.json")
        );
    }

    #[test]
    fn rejects_non_minecraft_data_path() {
        assert!(
            strip_minecraft_data_prefix(Path::new("assets/minecraft/lang/en_us.json")).is_none()
        );
    }

    #[test]
    fn strips_minecraft_lang_prefix() {
        let path = Path::new("assets/minecraft/lang/en_us.json");
        assert_eq!(
            strip_minecraft_lang_prefix(path).unwrap(),
            Path::new("en_us.json")
        );
    }

    #[test]
    fn strips_minecraft_lang_prefix_for_zh_cn() {
        let path = Path::new("assets/minecraft/lang/zh_cn.json");
        assert_eq!(
            strip_minecraft_lang_prefix(path).unwrap(),
            Path::new("zh_cn.json")
        );
    }

    #[test]
    fn rejects_non_minecraft_lang_path() {
        assert!(
            strip_minecraft_lang_prefix(Path::new("data/minecraft/tags/damage_type/is_fire.json"))
                .is_none()
        );
    }
}
