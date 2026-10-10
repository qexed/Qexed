use serde::Deserialize;

use crate::error::MojangDataError;

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
    #[serde(default)]
    java_version: Option<JavaVersion>,
}

#[derive(Debug, Deserialize)]
struct JavaVersion {
    #[serde(rename = "majorVersion")]
    major: u32,
}

#[derive(Debug, Deserialize)]
struct VersionDownloads {
    server: Option<DownloadInfo>,
    client: Option<DownloadInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct DownloadInfo {
    pub(super) sha1: String,
    pub(super) size: u64,
    pub(super) url: String,
}

pub(super) fn resolve_assets(
    client: &reqwest::blocking::Client,
    manifest_url: &str,
    version: &str,
) -> Result<Vec<(&'static str, DownloadInfo)>, MojangDataError> {
    let manifest = get_json::<VersionManifest>(client, manifest_url)?;
    let version_entry = manifest
        .versions
        .into_iter()
        .find(|entry| entry.id == version)
        .ok_or_else(|| MojangDataError::VersionNotFound(version.to_string()))?;

    let version_json = get_json::<VersionJson>(client, &version_entry.url)?;

    let mut assets = Vec::new();
    if let Some(server) = version_json.downloads.server {
        assets.push(("server", server));
    }
    if let Some(client) = version_json.downloads.client {
        assets.push(("client", client));
    }
    if assets.is_empty() {
        return Err(MojangDataError::MissingDownloads(version.to_string()));
    }
    Ok(assets)
}

/// 解析版本元数据中的 `javaVersion.majorVersion`（Mojang 官方要求的 Java 大版本）。
///
/// 快照版本可能缺失该字段，回退到 [`DEFAULT_JAVA_MAJOR`]。
pub(super) fn resolve_java_major(
    client: &reqwest::blocking::Client,
    manifest_url: &str,
    version: &str,
) -> Result<u32, MojangDataError> {
    let manifest = get_json::<VersionManifest>(client, manifest_url)?;
    let version_entry = manifest
        .versions
        .into_iter()
        .find(|entry| entry.id == version)
        .ok_or_else(|| MojangDataError::VersionNotFound(version.to_string()))?;

    let version_json = get_json::<VersionJson>(client, &version_entry.url)?;
    Ok(version_json
        .java_version
        .map(|java| java.major)
        .unwrap_or(DEFAULT_JAVA_MAJOR))
}

/// Mojang 元数据缺失 javaVersion 时的兜底大版本。
pub(super) const DEFAULT_JAVA_MAJOR: u32 = 25;

fn get_json<T>(client: &reqwest::blocking::Client, url: &str) -> Result<T, MojangDataError>
where
    T: for<'de> Deserialize<'de>,
{
    let response = client.get(url).send()?.error_for_status()?;
    Ok(response.json::<T>()?)
}
