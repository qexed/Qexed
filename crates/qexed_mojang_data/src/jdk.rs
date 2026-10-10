//! Temurin JRE 自动缓存：跨平台发现、下载、解压与版本校验。
//!
//! 优先使用系统 Java（config.java_path → JAVA_HOME → PATH），
//! 版本不满足时回退到从 Adoptium 下载与当前平台匹配的 Temurin JRE。
//!
//! 平台矩阵（image_type=jre, vendor=eclipse）：
//! - windows/x64: zip
//! - linux/x64, linux/aarch64, linux其他: tar.gz
//! - mac/x64, mac/aarch64: tar.gz（macOS 的 JRE 也是 tar.gz 而非 pkg）
//! - windows/aarch64: Temurin 无构建，报错并提示手动安装

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use sha2::{Digest, Sha256};

use crate::error::{IoCtx, MojangDataError};

/// Adoptium v3 跳转端点：307 到 GitHub Releases 的最新 GA 包。
const ADOPTIUM_BINARY_URL: &str = "https://api.adoptium.net/v3/binary/latest";

pub(super) struct Platform {
    pub(super) os: &'static str,
    pub(super) arch: &'static str,
}

/// 编译期平台探测。musl/Alpine 走 alpine-linux 变体。
pub(super) fn current_platform() -> Result<Platform, MojangDataError> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        return Err(MojangDataError::JdkPlatformUnsupported);
    };

    let arch = if cfg!(any(target_arch = "x86_64", target_arch = "x86")) {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(MojangDataError::JdkPlatformUnsupported);
    };

    Ok(Platform { os, arch })
}

/// 平台是否被 Temurin 覆盖。windows/aarch64 目前无官方构建。
fn platform_supported(platform: &Platform) -> bool {
    !matches!(
        (platform.os, platform.arch),
        ("windows", "aarch64") | ("windows", "arm")
    )
}

fn adoptium_url(platform: &Platform, major: u32) -> String {
    // alpine（musl）需要 os=alpine-linux 变体
    let os = if platform.os == "linux" && cfg!(target_env = "musl") {
        "alpine-linux"
    } else {
        platform.os
    };
    let arch = platform.arch;
    format!("{ADOPTIUM_BINARY_URL}/{major}/ga/{os}/{arch}/jre/hotspot/normal/eclipse")
}

/// 解析后的 Java 运行环境。
#[derive(Debug, Clone)]
pub(super) struct JavaRuntime {
    pub(super) java_bin: PathBuf,
    #[allow(dead_code)]
    pub(super) source: JavaSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JavaSource {
    Config,
    JavaHome,
    Path,
    Downloaded,
}

/// 确保一个满足 major 版本要求的 java 可执行文件，返回其路径与来源。
pub(super) fn ensure_java(
    config_java_path: &str,
    allow_download: bool,
    major: u32,
    cache_root: &Path,
) -> Result<JavaRuntime, MojangDataError> {
    // 1. 显式配置的 java_path：不校验也用（用户强制指定），但版本不符时告警。
    if !config_java_path.trim().is_empty() {
        let path = PathBuf::from(config_java_path.trim());
        match java_major_version(&path) {
            Ok(Some(actual)) if actual >= major => {
                return Ok(JavaRuntime { java_bin: path, source: JavaSource::Config });
            }
            Ok(Some(actual)) => log::warn!(
                "配置的 java 版本过低 (需要 >= {major}, 实际 {actual})，将尝试自动发现或下载"
            ),
            Ok(None) => log::warn!(
                "无法解析配置 java_path 的版本: {}，仍将使用它",
                path.display()
            ),
            Err(err) => log::warn!(
                "无法执行配置的 java ({}): {err}，将尝试自动发现或下载",
                path.display()
            ),
        }
        // 版本不符时也允许直接用配置路径？不：继续走发现链，最后再兜底用配置的。
    }

    // 2. JAVA_HOME
    if let Ok(home) = std::env::var("JAVA_HOME")
        && !home.trim().is_empty()
    {
        let candidate = java_bin_in(&PathBuf::from(home.trim()));
        if let Ok(Some(actual)) = java_major_version(&candidate)
            && actual >= major
        {
            return Ok(JavaRuntime { java_bin: candidate, source: JavaSource::JavaHome });
        }
    }

    // 3. PATH
    if let Some(candidate) = find_java_on_path() {
        if let Ok(Some(actual)) = java_major_version(&candidate)
            && actual >= major
        {
            return Ok(JavaRuntime { java_bin: candidate, source: JavaSource::Path });
        }
    }

    // 4. 已缓存的自下载 JRE（先于重新下载，且不依赖网络）
    let jre_root = cache_root.join("jre");
    let cached = jre_root.join(format!("java-{major}"));
    let cached_bin = java_bin_in(&cached);
    if let Ok(Some(actual)) = java_major_version(&cached_bin)
        && actual >= major
    {
        return Ok(JavaRuntime { java_bin: cached_bin, source: JavaSource::Downloaded });
    }

    // 5. 下载
    if !allow_download {
        if !config_java_path.trim().is_empty() {
            // 禁止下载时兜底使用用户配置（已告警过版本不符）。
            let path = PathBuf::from(config_java_path.trim());
            if path.is_file() {
                return Ok(JavaRuntime { java_bin: path, source: JavaSource::Config });
            }
        }
        return Err(MojangDataError::JavaNotFound { required_major: major });
    }

    let platform = current_platform()?;
    if !platform_supported(&platform) {
        return Err(MojangDataError::JdkPlatformUnsupportedDetail(format!(
            "{}/{}",
            platform.os, platform.arch
        )));
    }

    let target = jre_root.join(format!("java-{major}"));
    download_jre(&platform, major, &target)?;

    let bin = java_bin_in(&target);
    let actual = java_major_version(&bin)?.unwrap_or(0);
    if actual < major {
        return Err(MojangDataError::JdkVerifyFailed {
            expected: major,
            actual: actual.to_string(),
            path: bin,
        });
    }
    log::info!(
        "Temurin JRE 缓存就绪: version={major}, path={}",
        target.display()
    );
    Ok(JavaRuntime { java_bin: bin, source: JavaSource::Downloaded })
}

fn java_bin_in(java_home: &Path) -> PathBuf {
    let bin = java_home.join("bin");
    if cfg!(target_os = "windows") {
        bin.join("java.exe")
    } else {
        bin.join("java")
    }
}

fn find_java_on_path() -> Option<PathBuf> {
    let name = if cfg!(target_os = "windows") { "java.exe" } else { "java" };
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 解析 java 主版本。java 8 及更早输出 "1.8.0_503"（取 8），新版本输出 "25.0.4"（取 25）。
fn java_major_version(java_bin: &Path) -> Result<Option<u32>, MojangDataError> {
    if !java_bin.is_file() {
        return Ok(None);
    }
    let output = Command::new(java_bin)
        .arg("-version")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .io_ctx(format!("无法执行 java: {}", java_bin.display()))?;
    let text = String::from_utf8_lossy(&output.stderr);
    if !text.contains("version") {
        let stdout_text = String::from_utf8_lossy(&output.stdout);
        return parse_java_major(&stdout_text);
    }
    parse_java_major(&text)
}

fn parse_java_major(text: &str) -> Result<Option<u32>, MojangDataError> {
    // 形如: java version "25.0.4" 2025-10-21 / openjdk version "1.8.0_503"
    let Some(start) = text.find('"') else {
        return Ok(None);
    };
    let rest = &text[start + 1..];
    let Some(end) = rest.find('"') else {
        return Ok(None);
    };
    let version = &rest[..end];
    let mut parts = version.split(['.', '_', '-', '+']);
    let Some(first) = parts.next() else {
        return Ok(None);
    };
    let major = first.parse::<u32>().unwrap_or_else(|_| {
        // 解析失败回退 0（后续按不满足处理）
        0
    });
    if major == 1 {
        // "1.8.0" 形式：真正的版本在第二段
        if let Some(second) = parts.next() {
            if let Ok(v) = second.parse::<u32>() {
                return Ok(Some(v));
            }
        }
        return Ok(None);
    }
    if major == 0 {
        return Ok(None);
    }
    Ok(Some(major))
}

/// JRE 下载专用 client：连接超时独立于总超时（黑洞网络下 30s 快速失败换源）。
fn jre_http_client() -> Result<reqwest::blocking::Client, MojangDataError> {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(600))
        .user_agent(format!("qexed/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(MojangDataError::Http)
}

/// 带重试的 GET（连接重置/瞬时网络故障常见，尤其部分地区访问 CDN）。
/// 每源最多 2 次：快速失败尽快切换镜像。
fn fetch_with_retry(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<reqwest::blocking::Response, MojangDataError> {
    const ATTEMPTS: usize = 2;
    let mut last_err = None;
    for attempt in 1..=ATTEMPTS {
        let request = client
            .get(url)
            .timeout(std::time::Duration::from_secs(120));
        match request.send().and_then(|r| r.error_for_status()) {
            Ok(response) => return Ok(response),
            Err(err) => {
                log::warn!("下载失败 ({attempt}/{ATTEMPTS}): {url}: {err}");
                last_err = Some(MojangDataError::Http(err));
                if attempt < ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}

fn download_jre(
    platform: &Platform,
    major: u32,
    target: &Path,
) -> Result<(), MojangDataError> {
    let marker = target.with_extension("ready");
    if marker.is_file() {
        return Ok(());
    }

    log::info!("开始下载 Temurin JRE {major} ({}/{})", platform.os, platform.arch);

    // 第一步：解析官方 307 跳转拿到 GitHub 文件名（轻量请求，只到 Cloudflare 边缘）。
    let official_url = adoptium_url(platform, major);
    let github_location = resolve_redirect(&official_url)?;
    let file_name = github_location
        .rsplit('/')
        .next()
        .map(|name| urldecode(name))
        .unwrap_or_default();

    // 第二步：多源回退下载（官方 GitHub → 清华 TUNA 镜像）。
    let mut sources = vec![github_location.clone()];
    if !file_name.is_empty() {
        sources.push(tuna_url(platform, major, &file_name));
    }
    let download_client = jre_http_client()?;

    // 第三步：官方 sha256 侧车（拿不到则跳过校验）。
    let expected = fetch_sha256_sidecar(&download_client, &github_location);

    let (mut response, used_url) = fetch_first_available(&download_client, &sources)?;

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).io_ctx(format!(
            "无法创建 JRE 缓存目录: {}",
            parent.display()
        ))?;
    }
    let archive = target.with_extension("downloading");
    let mut file = std::fs::File::create(&archive).io_ctx(format!(
        "无法创建 JRE 临时文件: {}",
        archive.display()
    ))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        use std::io::Read;
        let read = response
            .read(&mut buffer)
            .io_ctx(format!("下载 Temurin JRE 失败: {used_url}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        std::io::Write::write_all(&mut file, &buffer[..read])?;
    }
    drop(file);
    let digest = hex::encode(hasher.finalize());

    if !expected.is_empty() && !expected.eq_ignore_ascii_case(&digest) {
        std::fs::remove_file(&archive).ok();
        return Err(MojangDataError::Sha256Mismatch {
            expected,
            actual: digest,
        });
    }

    // 解压到临时目录再原子改名
    let extract_tmp = target.with_extension("extracting");
    if extract_tmp.exists() {
        std::fs::remove_dir_all(&extract_tmp).io_ctx(format!(
            "无法清理 JRE 解压临时目录: {}",
            extract_tmp.display()
        ))?;
    }
    std::fs::create_dir_all(&extract_tmp)?;
    extract_archive(&archive, &extract_tmp)?;

    // Temurin 包内是单个顶层目录（jdk-25.0.4+1-jre/...），把它提升为 target
    let entries: Vec<_> = std::fs::read_dir(&extract_tmp)
        .io_ctx("无法读取 JRE 解压目录")?
        .filter_map(|e| e.ok())
        .collect();
    if entries.len() == 1 && entries[0].path().is_dir() {
        let inner = entries[0].path();
        std::fs::rename(&inner, target).io_ctx(format!(
            "无法安装 JRE 到缓存: {}",
            target.display()
        ))?;
        std::fs::remove_dir_all(&extract_tmp).ok();
    } else {
        std::fs::rename(&extract_tmp, target).io_ctx(format!(
            "无法安装 JRE 到缓存: {}",
            target.display()
        ))?;
    }

    std::fs::remove_file(&archive).ok();
    // ready 标记记录版本，便于排查
    std::fs::write(&marker, format!("major={major}\nsha256={digest}\n")).io_ctx(
        "无法写入 JRE ready 标记",
    )?;
    Ok(())
}

/// 请求并读取 307 Location（不跟随跳转；Adoptium API 端点本身在 Cloudflare，可达性好）。
fn resolve_redirect(url: &str) -> Result<String, MojangDataError> {
    let no_redirect = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(format!("qexed/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(MojangDataError::Http)?;
    let response = fetch_with_retry(&no_redirect, url)?;
    let status = response.status();
    if !status.is_redirection() {
        return Err(MojangDataError::JdkRedirect(format!(
            "expected 3xx from Adoptium API, got {status}"
        )));
    }
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| MojangDataError::JdkRedirect("missing Location header".into()))
}

/// 清华 TUNA Adoptium 镜像路径约定：/Adoptium/{major}/jre/{arch}/{os}/{file}。
/// os 名称映射：windows→windows, linux→linux (musl→alpine-linux), mac→mac。
fn tuna_url(platform: &Platform, major: u32, file_name: &str) -> String {
    let os = if platform.os == "linux" && cfg!(target_env = "musl") {
        "alpine-linux"
    } else {
        platform.os
    };
    format!(
        "https://mirrors.tuna.tsinghua.edu.cn/Adoptium/{major}/jre/{}/{os}/{file_name}",
        platform.arch
    )
}

/// 逐源尝试下载，第一个成功的胜出；全部失败时返回最后一个错误。
fn fetch_first_available(
    client: &reqwest::blocking::Client,
    sources: &[String],
) -> Result<(reqwest::blocking::Response, String), MojangDataError> {
    let mut last_err = None;
    for url in sources {
        log::info!("JRE 下载源: {url}");
        match fetch_with_retry(client, url) {
            Ok(response) => return Ok((response, url.clone())),
            Err(err) => {
                log::warn!("JRE 下载源不可用，尝试下一个: {url}: {err}");
                last_err = Some((url.clone(), err));
            }
        }
    }
    let (url, err) = last_err.expect("at least one source");
    Err(MojangDataError::JdkDownloadAllFailed {
        url,
        source: Box::new(err),
    })
}

/// 极简 percent-decode（GitHub 文件名带 %2B 等转义）。
fn urldecode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() + 1 && i + 2 < bytes.len() + 1 => {
                let hex_pair = bytes
                    .get(i + 1..i + 3)
                    .and_then(|pair| std::str::from_utf8(pair).ok())
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok());
                match hex_pair {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b'+');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// GitHub Releases 的 sha256 侧车文件：主包 URL + ".sha256.txt"。
/// 拿不到时返回空串（跳过校验，靠解压后 java -version 兜底验证）。
fn fetch_sha256_sidecar(client: &reqwest::blocking::Client, package_url: &str) -> String {
    let sidecar_url = format!("{package_url}.sha256.txt");
    let response = match fetch_with_retry(client, &sidecar_url) {
        Ok(response) => response,
        Err(err) => {
            log::warn!("无法获取 JRE sha256 校验文件，跳过校验: {err}");
            return String::new();
        }
    };
    match response.text() {
        Ok(text) => text
            .trim()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase(),
        Err(err) => {
            log::warn!("读取 JRE sha256 校验文件失败，跳过校验: {err}");
            String::new()
        }
    }
}

fn extract_archive(archive: &Path, dest: &Path) -> Result<(), MojangDataError> {
    // windows 包是 zip（PK 魔数），linux/mac 是 tar.gz（0x1f8b）。
    // 统一用魔数判断，避免依赖文件扩展名。
    let mut magic = [0u8; 2];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(archive)?;
        let _ = f.read(&mut magic);
    }
    if magic == [0x50, 0x4b] {
        extract_zip(archive, dest)
    } else {
        extract_tar_gz(archive, dest)
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), MojangDataError> {
    let file = std::fs::File::open(archive)?;
    let reader = std::io::BufReader::new(file);
    let mut zip = zip::ZipArchive::new(reader)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let target = dest.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&target)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), MojangDataError> {
    let file = std::fs::File::open(archive)?;
    let reader = std::io::BufReader::new(file);
    let gz = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(gz);
    archive.set_preserve_permissions(true);
    archive.unpack(dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_java_version() {
        assert_eq!(
            parse_java_major("java version \"25.0.4\" 2025-10-21").unwrap(),
            Some(25)
        );
    }

    #[test]
    fn parses_legacy_java_version() {
        assert_eq!(
            parse_java_major("java version \"1.8.0_503\"").unwrap(),
            Some(8)
        );
    }

    #[test]
    fn parses_openjdk_prefix() {
        assert_eq!(
            parse_java_major("openjdk version \"17.0.2\" 2022-01-18").unwrap(),
            Some(17)
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_java_major("not a version line").unwrap(), None);
    }

    #[test]
    fn adoptium_url_shape() {
        let p = Platform { os: "windows", arch: "x64" };
        assert_eq!(
            adoptium_url(&p, 25),
            "https://api.adoptium.net/v3/binary/latest/25/ga/windows/x64/jre/hotspot/normal/eclipse"
        );
    }
}
