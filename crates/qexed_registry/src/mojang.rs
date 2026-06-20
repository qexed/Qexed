use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};

type RegistryConfig = qexed_config::app::qexed_registry::Registry;
const JAVA_COMMAND: &str = "java";
const MIN_JAVA_MAJOR_VERSION: u32 = 25;
const DOWNLOAD_BUFFER_SIZE: usize = 64 * 1024;
const DOWNLOAD_PROGRESS_STEP: u64 = 5;
const DOWNLOAD_LOCK_MAX_ATTEMPTS: usize = 600;
const DOWNLOAD_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(100);
const DOWNLOAD_LOCK_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const DOWNLOAD_LOCK_STALE_AFTER: Duration = Duration::from_secs(15);
static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();
static REPORTS_ROOT: OnceLock<PathBuf> = OnceLock::new();

pub async fn init(config: &RegistryConfig) -> Result<PathBuf> {
    let config = BlockingConfig::from(config);
    let roots = tokio::task::spawn_blocking(move || init_blocking(&config))
        .await
        .context("Mojang 数据初始化阻塞任务失败")??;
    set_data_root(roots.data_root.clone());
    set_reports_root(roots.reports_root);
    Ok(roots.data_root)
}

pub(crate) fn data_root() -> Option<PathBuf> {
    DATA_ROOT.get().cloned()
}

pub(crate) fn reports_root() -> Option<PathBuf> {
    REPORTS_ROOT.get().cloned()
}

fn set_data_root(path: PathBuf) {
    if DATA_ROOT.set(path).is_err() {
        tklog::warn!("Mojang data root 已初始化，忽略重复配置");
    }
}

fn set_reports_root(path: PathBuf) {
    if REPORTS_ROOT.set(path).is_err() {
        tklog::warn!("Mojang reports root 已初始化，忽略重复配置");
    }
}

struct CacheRoots {
    data_root: PathBuf,
    reports_root: PathBuf,
}

struct BlockingConfig {
    version_manifest_url: String,
    cache_dir: String,
    data_marker: String,
    http_timeout: u64,
}

impl From<&RegistryConfig> for BlockingConfig {
    fn from(config: &RegistryConfig) -> Self {
        Self {
            version_manifest_url: config.version_manifest_url.clone(),
            cache_dir: config.cache_dir.clone(),
            data_marker: config.data_marker.clone(),
            http_timeout: config.http_timeout,
        }
    }
}

fn init_blocking(config: &BlockingConfig) -> Result<CacheRoots> {
    let cache_root =
        resolve_cache_path(PathBuf::from(&config.cache_dir)).join(qexed_config::MC_VERSION);
    let data_root = cache_root.join("data/minecraft");
    let reports_root = cache_root.join("generated/reports");

    if data_root_ready(&data_root, &config.data_marker) && reports_root_ready(&reports_root) {
        return Ok(CacheRoots {
            data_root,
            reports_root,
        });
    }

    tklog::info!("Mojang Jar 准备下载，用于提取注册表资源");
    let lock_path = cache_root.join(".download.lock");
    let _lock = DownloadLock::acquire(&lock_path)?;
    if data_root_ready(&data_root, &config.data_marker) && reports_root_ready(&reports_root) {
        tklog::info!("Mojang 数据缓存已由其他任务完成");
        return Ok(CacheRoots {
            data_root,
            reports_root,
        });
    }

    std::fs::create_dir_all(&cache_root)
        .with_context(|| format!("无法创建 Mojang 缓存目录: {}", cache_root.display()))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(config.http_timeout))
        .user_agent(format!("qexed/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("无法创建 Mojang 下载客户端")?;

    let assets = resolve_assets(&client, config)?;
    let mut errors = Vec::new();
    for (kind, asset) in assets {
        let jar_path = cache_root.join(format!("{kind}.jar"));
        download_if_needed(&client, &asset, &jar_path)?;
        match kind {
            "client" if !data_root_ready(&data_root, &config.data_marker) => {
                if let Err(err) =
                    extract_minecraft_data(&jar_path, &cache_root, &config.data_marker)
                {
                    errors.push(format!("{kind}: {err:#}"));
                }
            }
            "server" if !reports_root_ready(&reports_root) => {
                if let Err(err) = generate_minecraft_reports(&jar_path, &cache_root, &reports_root)
                {
                    errors.push(format!("{kind}: {err:#}"));
                }
            }
            _ => {}
        }
    }

    if !data_root_ready(&data_root, &config.data_marker) || !reports_root_ready(&reports_root) {
        anyhow::bail!(
            "Mojang jar 中无法提取 registry data 或生成 reports，已尝试: {}",
            errors.join("; ")
        );
    }

    cleanup_downloaded_jars(&cache_root);
    Ok(CacheRoots {
        data_root,
        reports_root,
    })
}

fn resolve_cache_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    std::env::current_dir()
        .ok()
        .filter(|path| path.join("Cargo.toml").is_file() && path.join("crates").is_dir())
        .unwrap_or_else(workspace_root)
        .join(path)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn data_root_ready(data_root: &Path, data_marker: &str) -> bool {
    data_root.join(data_marker).is_file()
        && data_root.join("dimension_type").is_dir()
        && data_root.join("damage_type").is_dir()
        && data_root.join("tags/damage_type").is_dir()
}

fn reports_root_ready(reports_root: &Path) -> bool {
    reports_root.join("registries.json").is_file() && reports_root.join("blocks.json").is_file()
}

fn resolve_assets(
    client: &reqwest::blocking::Client,
    config: &BlockingConfig,
) -> Result<Vec<(&'static str, DownloadInfo)>> {
    let version = qexed_config::MC_VERSION;
    let manifest = get_json::<VersionManifest>(client, &config.version_manifest_url)?;
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
            Ok(()) => {
                tklog::info!(format!(
                    "Mojang jar 缓存命中: path={}, size={}",
                    target.display(),
                    format_bytes(asset.size)
                ));
                return Ok(());
            }
            Err(err) => {
                tklog::warn!(format!(
                    "Mojang 缓存 jar 校验失败，将重新下载: path={}, error={:#}",
                    target.display(),
                    err
                ));
            }
        }
    }

    let response = client
        .get(&asset.url)
        .send()
        .with_context(|| format!("Mojang jar 下载失败: {}", asset.url))?
        .error_for_status()
        .with_context(|| format!("Mojang jar 返回非成功状态: {}", asset.url))?;

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建 Mojang jar 缓存目录: {}", parent.display()))?;
    }
    let tmp = target.with_extension("jar.tmp");
    let mut output = BufWriter::new(
        File::create(&tmp)
            .with_context(|| format!("无法写入 Mojang jar 临时缓存: {}", tmp.display()))?,
    );
    let total_size = expected_download_size(asset, response.content_length());
    let mut response = response;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; DOWNLOAD_BUFFER_SIZE];
    let mut downloaded = 0u64;
    let mut next_progress = 0u64;

    tklog::info!(format!(
        "开始下载 Mojang jar: path={}, size={}",
        target.display(),
        format_bytes(total_size)
    ));

    loop {
        let read = response
            .read(&mut buffer)
            .context("无法读取 Mojang jar 响应体")?;
        if read == 0 {
            break;
        }

        output
            .write_all(&buffer[..read])
            .with_context(|| format!("无法写入 Mojang jar 临时缓存: {}", tmp.display()))?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        log_download_progress(downloaded, total_size, &mut next_progress);
    }
    output
        .flush()
        .with_context(|| format!("无法写入 Mojang jar 临时缓存: {}", tmp.display()))?;

    if asset.size > 0 && downloaded != asset.size {
        anyhow::bail!(
            "Mojang jar 大小不匹配: expected={}, actual={}",
            asset.size,
            downloaded
        );
    }
    verify_digest(hasher.finalize().as_slice(), &asset.sha1)?;

    std::fs::rename(&tmp, target)
        .with_context(|| format!("无法保存 Mojang jar 缓存: {}", target.display()))?;
    tklog::info!(format!(
        "Mojang jar 下载完成: path={}, size={}",
        target.display(),
        format_bytes(downloaded)
    ));
    Ok(())
}

fn expected_download_size(asset: &DownloadInfo, content_length: Option<u64>) -> u64 {
    if asset.size > 0 {
        asset.size
    } else {
        content_length.unwrap_or_default()
    }
}

fn log_download_progress(downloaded: u64, total_size: u64, next_progress: &mut u64) {
    if total_size == 0 {
        return;
    }

    let progress = (downloaded.saturating_mul(100) / total_size).min(100);
    if progress < *next_progress && progress < 100 {
        return;
    }

    tklog::info!(format!(
        "Mojang jar 下载进度: {}% ({}/{})",
        progress,
        format_bytes(downloaded),
        format_bytes(total_size)
    ));
    *next_progress = progress.saturating_add(DOWNLOAD_PROGRESS_STEP);
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    if bytes == 0 {
        return "unknown".to_string();
    }

    let mut size = bytes as f64;
    let mut unit = 0usize;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.2} {}", UNITS[unit])
    }
}

fn cleanup_downloaded_jars(cache_root: &Path) {
    for name in ["server.jar", "client.jar"] {
        let path = cache_root.join(name);
        if let Err(err) = std::fs::remove_file(&path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            tklog::warn!(format!(
                "提取 Mojang 数据后删除 jar 失败: path={}, error={}",
                path.display(),
                err
            ));
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

fn verify_digest(actual: &[u8], expected: &str) -> Result<()> {
    let actual = hex::encode(actual);
    if !actual.eq_ignore_ascii_case(expected) {
        anyhow::bail!("SHA1 不匹配: expected={expected}, actual={actual}");
    }
    Ok(())
}

fn extract_minecraft_data(jar_path: &Path, cache_root: &Path, data_marker: &str) -> Result<()> {
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
        tmp_root.join("minecraft").join(data_marker),
        total_extracted.to_string(),
    )
    .context("无法写入 Mojang 数据缓存完成标记")?;

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

    tklog::info!(format!(
        "Mojang 注册表缓存: version={}, registry_files={}, lang_files={}, path={}",
        qexed_config::MC_VERSION,
        extracted,
        lang_extracted,
        data_root.display()
    ));
    Ok(())
}

fn generate_minecraft_reports(
    jar_path: &Path,
    cache_root: &Path,
    reports_root: &Path,
) -> Result<()> {
    let jar_path = jar_path
        .canonicalize()
        .with_context(|| format!("无法解析 Mojang server jar 路径: {}", jar_path.display()))?;
    let cache_root = cache_root
        .canonicalize()
        .with_context(|| format!("无法解析 Mojang 缓存目录: {}", cache_root.display()))?;
    let output_root = cache_root.join("generated");
    let tmp_output_root = cache_root.join("generated.tmp");
    if tmp_output_root.exists() {
        std::fs::remove_dir_all(&tmp_output_root).with_context(|| {
            format!(
                "无法清理 Mojang reports 临时目录: {}",
                tmp_output_root.display()
            )
        })?;
    }
    if output_root.exists() {
        std::fs::remove_dir_all(&output_root).with_context(|| {
            format!("无法清理旧 Mojang reports 目录: {}", output_root.display())
        })?;
    }
    std::fs::create_dir_all(&tmp_output_root).with_context(|| {
        format!(
            "无法创建 Mojang reports 临时目录: {}",
            tmp_output_root.display()
        )
    })?;

    tklog::info!(format!(
        "开始生成 Mojang reports: jar={}, output={}",
        jar_path.display(),
        tmp_output_root.display()
    ));
    ensure_java_runtime()?;
    let status = Command::new(JAVA_COMMAND)
        .current_dir(&cache_root)
        .arg("-DbundlerMainClass=net.minecraft.data.Main")
        .arg("-jar")
        .arg(&jar_path)
        .arg("--reports")
        .arg("--output")
        .arg(&tmp_output_root)
        .status()
        .with_context(|| "无法启动 Java 生成 Mojang reports")?;
    if !status.success() {
        anyhow::bail!("Mojang reports 生成失败: status={status}");
    }

    let generated_reports = tmp_output_root.join("reports");
    if !reports_root_ready(&generated_reports) {
        anyhow::bail!(
            "Mojang reports 生成结果缺少 registries.json 或 blocks.json: {}",
            generated_reports.display()
        );
    }

    std::fs::rename(&tmp_output_root, &output_root).with_context(|| {
        format!(
            "无法安装 Mojang reports 生成目录: {}",
            output_root.display()
        )
    })?;
    if reports_root_ready(reports_root) {
        tklog::info!(format!(
            "Mojang reports 已生成: path={}",
            reports_root.display()
        ));
        Ok(())
    } else {
        anyhow::bail!("Mojang reports 安装后仍不可用: {}", reports_root.display())
    }
}

fn ensure_java_runtime() -> Result<()> {
    let output = Command::new(JAVA_COMMAND)
        .arg("-version")
        .output()
        .with_context(|| {
            format!(
                "无法启动 Java。Minecraft {} 的 Mojang reports 生成需要 Java {}+，请安装 JDK 并确保 PATH 中的 java 可用",
                qexed_config::MC_VERSION,
                MIN_JAVA_MAJOR_VERSION
            )
        })?;

    let version_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let java_major = parse_java_major_version(&version_output).with_context(|| {
        format!(
            "无法识别 Java 版本。Minecraft {} 的 Mojang reports 生成需要 Java {}+，java -version 输出: {}",
            qexed_config::MC_VERSION,
            MIN_JAVA_MAJOR_VERSION,
            compact_command_output(&version_output)
        )
    })?;

    if java_major < MIN_JAVA_MAJOR_VERSION {
        anyhow::bail!(
            "当前 Java 主版本为 {}，但 Minecraft {} 的 Mojang reports 生成需要 Java {}+。请更新 JDK，并确保 PATH 中的 java 指向正确版本",
            java_major,
            qexed_config::MC_VERSION,
            MIN_JAVA_MAJOR_VERSION
        );
    }

    tklog::info!(format!(
        "Java 版本检测通过: major={}, required={}+",
        java_major, MIN_JAVA_MAJOR_VERSION
    ));
    Ok(())
}

fn parse_java_major_version(output: &str) -> Option<u32> {
    for token in output.split_whitespace() {
        let version = token.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ',');
        let Some(first) = version.split(['.', '-', '+']).next() else {
            continue;
        };
        let Ok(first_number) = first.parse::<u32>() else {
            continue;
        };

        if first_number == 1 {
            if let Some(second) = version.split('.').nth(1)
                && let Ok(legacy_major) = second.parse::<u32>()
            {
                return Some(legacy_major);
            }
        } else {
            return Some(first_number);
        }
    }
    None
}

fn compact_command_output(output: &str) -> String {
    let compact = output.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_OUTPUT_LEN: usize = 240;
    let mut chars = compact.chars();
    let shortened: String = chars.by_ref().take(MAX_OUTPUT_LEN).collect();
    if chars.next().is_none() {
        compact
    } else {
        format!("{shortened}...")
    }
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
    heartbeat_stop: Arc<AtomicBool>,
    heartbeat: Option<JoinHandle<()>>,
}

impl DownloadLock {
    fn acquire(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建 Mojang 下载锁目录: {}", parent.display()))?;
        }

        for _ in 0..DOWNLOAD_LOCK_MAX_ATTEMPTS {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut file) => {
                    write_download_lock(&mut file)?;
                    let path = path.to_path_buf();
                    let heartbeat_stop = Arc::new(AtomicBool::new(false));
                    let heartbeat =
                        start_download_lock_heartbeat(path.clone(), Arc::clone(&heartbeat_stop));
                    return Ok(Self {
                        path,
                        heartbeat_stop,
                        heartbeat: Some(heartbeat),
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    if remove_stale_download_lock(path)? {
                        continue;
                    }
                    thread::sleep(DOWNLOAD_LOCK_RETRY_INTERVAL);
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
        self.heartbeat_stop.store(true, Ordering::Relaxed);
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

fn start_download_lock_heartbeat(path: PathBuf, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            thread::sleep(DOWNLOAD_LOCK_HEARTBEAT_INTERVAL);
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if let Err(err) = refresh_download_lock(&path) {
                tklog::warn!(format!(
                    "刷新 Mojang 下载锁失败: path={}, error={:#}",
                    path.display(),
                    err
                ));
                break;
            }
        }
    })
}

fn refresh_download_lock(path: &Path) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("无法打开 Mojang 下载锁: {}", path.display()))?;
    write_download_lock(&mut file)
}

fn write_download_lock(file: &mut File) -> Result<()> {
    writeln!(file, "pid={}", std::process::id()).context("无法写入 Mojang 下载锁进程 ID")?;
    writeln!(file, "version={}", qexed_config::MC_VERSION).context("无法写入 Mojang 下载锁版本")?;
    file.flush().context("无法刷新 Mojang 下载锁")
}

fn remove_stale_download_lock(path: &Path) -> Result<bool> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("无法读取 Mojang 下载锁状态: {}", path.display()));
        }
    };

    let Some(age) = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
    else {
        return Ok(false);
    };
    if age < DOWNLOAD_LOCK_STALE_AFTER {
        return Ok(false);
    }

    tklog::warn!(format!(
        "检测到过期 Mojang 下载锁，将重新获取: path={}, age={}s",
        path.display(),
        age.as_secs()
    ));
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("无法删除过期 Mojang 下载锁: {}", path.display()))
        }
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

    use super::{
        compact_command_output, parse_java_major_version, strip_minecraft_data_prefix,
        strip_minecraft_lang_prefix,
    };

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

    #[test]
    fn parses_modern_java_version() {
        assert_eq!(
            parse_java_major_version(
                r#"java version "25.0.1" 2026-10-21
Java(TM) SE Runtime Environment"#
            ),
            Some(25)
        );
    }

    #[test]
    fn parses_modern_java_runtime_build_version() {
        assert_eq!(
            parse_java_major_version(r#"openjdk version "25-ea" 2026-09-15"#),
            Some(25)
        );
    }

    #[test]
    fn parses_legacy_java_version() {
        assert_eq!(
            parse_java_major_version(r#"java version "1.8.0_402""#),
            Some(8)
        );
    }

    #[test]
    fn rejects_unrecognized_java_version() {
        assert_eq!(parse_java_major_version("not a java version"), None);
    }

    #[test]
    fn compacts_command_output() {
        assert_eq!(compact_command_output("a\n b\tc"), "a b c");
    }
}
