use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{ResourcePack, ResourcePackSource};
use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[derive(Clone)]
pub struct ResourcePackManager {
    state: ResourcePackState,
}

#[derive(Clone)]
enum ResourcePackState {
    Disabled,
    Url,
    ObjectStorage,
    Local(LocalResourcePack),
}

#[derive(Clone)]
struct LocalResourcePack {
    route: String,
    bind: std::net::SocketAddr,
    public_port: u16,
    hash: String,
    bytes: std::sync::Arc<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackOffer {
    pub url: String,
    pub hash: String,
}

impl ResourcePackManager {
    pub async fn from_config(config: &ResourcePack) -> Result<Self> {
        if !config.enable {
            return Ok(Self {
                state: ResourcePackState::Disabled,
            });
        }

        match config.source {
            ResourcePackSource::Url => Ok(Self {
                state: ResourcePackState::Url,
            }),
            ResourcePackSource::ObjectStorage => Ok(Self {
                state: ResourcePackState::ObjectStorage,
            }),
            ResourcePackSource::Local => Self::local(config).await,
        }
    }

    async fn local(config: &ResourcePack) -> Result<Self> {
        let path = std::path::Path::new(config.path.trim());
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("无法读取本地资源包文件 {}", path.display()))?;
        let hash = if config.hash.trim().is_empty() {
            sha1_hex(&bytes)
        } else {
            config.hash.trim().to_string()
        };
        let bind = bind_download_addr(&config.download_bind)?;
        let route = resource_pack_route(config.id);

        Ok(Self {
            state: ResourcePackState::Local(LocalResourcePack {
                route,
                bind,
                public_port: bind.port(),
                hash,
                bytes: std::sync::Arc::new(bytes),
            }),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        let ResourcePackState::Local(local) = &mut self.state else {
            return Ok(());
        };

        let listener = TcpListener::bind(local.bind)
            .await
            .with_context(|| format!("无法监听资源包下载地址 {}", local.bind))?;
        local.public_port = listener.local_addr()?.port();
        let served = local.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let served = served.clone();
                        tokio::spawn(async move {
                            if let Err(err) = handle_download(stream, served).await {
                                log::debug!("资源包下载请求处理失败: {err}");
                            }
                        });
                    }
                    Err(err) => {
                        log::warn!("资源包下载服务 accept 失败: {err}");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub fn offer(&self, config: &ResourcePack, login_host: &str) -> Option<ResourcePackOffer> {
        match &self.state {
            ResourcePackState::Disabled => None,
            ResourcePackState::Url => {
                let url = config.url.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(ResourcePackOffer {
                        url: url.to_string(),
                        hash: config.hash.trim().to_string(),
                    })
                }
            }
            ResourcePackState::ObjectStorage => {
                object_storage_download_url(config).map(|url| ResourcePackOffer {
                    url,
                    hash: config.hash.trim().to_string(),
                })
            }
            ResourcePackState::Local(local) => Some(ResourcePackOffer {
                url: local_download_url(config, login_host, local),
                hash: local.hash.clone(),
            }),
        }
    }
}

fn object_storage_download_url(config: &ResourcePack) -> Option<String> {
    let storage = &config.object_storage;
    let object_key = storage.object_key.trim().trim_start_matches('/');
    if object_key.is_empty() {
        return None;
    }

    let public_base_url = storage.public_base_url.trim();
    if !public_base_url.is_empty() {
        return Some(join_url_path(public_base_url, object_key));
    }

    let endpoint = storage.endpoint.trim();
    let bucket = storage.bucket.trim();
    if endpoint.is_empty() || bucket.is_empty() {
        return None;
    }

    let endpoint = endpoint_with_scheme(endpoint);
    if storage.force_path_style {
        Some(join_url_path(
            &join_url_path(&endpoint, bucket.trim_matches('/')),
            object_key,
        ))
    } else {
        Some(join_url_path(
            &virtual_host_endpoint(&endpoint, bucket),
            object_key,
        ))
    }
}

fn endpoint_with_scheme(endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.trim_end_matches('/').to_string()
    } else {
        format!("https://{}", endpoint.trim_end_matches('/'))
    }
}

fn virtual_host_endpoint(endpoint: &str, bucket: &str) -> String {
    if let Some(rest) = endpoint.strip_prefix("https://") {
        format!("https://{bucket}.{}", rest.trim_start_matches('/'))
    } else if let Some(rest) = endpoint.strip_prefix("http://") {
        format!("http://{bucket}.{}", rest.trim_start_matches('/'))
    } else {
        format!("https://{bucket}.{}", endpoint.trim_start_matches('/'))
    }
}

fn join_url_path(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn bind_download_addr(bind: &str) -> Result<std::net::SocketAddr> {
    bind.trim()
        .parse()
        .with_context(|| format!("资源包下载监听地址无效: {bind}"))
}

fn local_download_url(
    config: &ResourcePack,
    login_host: &str,
    local: &LocalResourcePack,
) -> String {
    let host = config.download_host.trim();
    if host.starts_with("http://") || host.starts_with("https://") {
        let base = host.trim_end_matches('/');
        return format!("{base}{}", local.route);
    }

    let host = if host.is_empty() {
        sanitize_login_host(login_host)
    } else {
        host.to_string()
    };

    let host = format_url_host(&host);
    if host_has_explicit_port(&host) {
        format!("http://{host}{}", local.route)
    } else {
        format!("http://{}:{}{}", host, local.public_port, local.route)
    }
}

fn sanitize_login_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.');
    if host.is_empty() || host == "0.0.0.0" || host == "::" {
        "127.0.0.1".to_string()
    } else {
        host.to_string()
    }
}

fn format_url_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        let colon_count = host.chars().filter(|ch| *ch == ':').count();
        if colon_count == 1 {
            host.to_string()
        } else {
            format!("[{host}]")
        }
    } else {
        host.to_string()
    }
}

fn host_has_explicit_port(host: &str) -> bool {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.contains("]:");
    }

    let Some((_, port)) = host.rsplit_once(':') else {
        return false;
    };
    port.chars().all(|ch| ch.is_ascii_digit())
}

fn resource_pack_route(id: uuid::Uuid) -> String {
    format!("/resource-pack/{id}.zip")
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

async fn handle_download(
    mut stream: tokio::net::TcpStream,
    local: LocalResourcePack,
) -> Result<()> {
    let request_bytes = read_http_headers(&mut stream).await?;
    let request = std::str::from_utf8(&request_bytes).unwrap_or_default();
    let path = request_path(request);

    if path == Some(local.route.as_str()) {
        write_response(&mut stream, 200, "OK", "application/zip", &local.bytes).await
    } else {
        write_response(
            &mut stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
        )
        .await
    }
}

async fn read_http_headers(stream: &mut tokio::net::TcpStream) -> Result<Vec<u8>> {
    const MAX_HEADER_SIZE: usize = 8192;

    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let size = stream.read(&mut buffer).await?;
        if size == 0 {
            break;
        }

        request.extend_from_slice(&buffer[..size]);
        if request.windows(4).any(|window| window == b"\r\n\r\n")
            || request.windows(2).any(|window| window == b"\n\n")
            || request.len() >= MAX_HEADER_SIZE
        {
            break;
        }
    }

    Ok(request)
}

fn request_path(request: &str) -> Option<&str> {
    let request_line = request.lines().next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    if method == "GET" {
        request_target_path(target)
    } else {
        None
    }
}

fn request_target_path(target: &str) -> Option<&str> {
    let path = if target.starts_with('/') {
        target
    } else if let Some(rest) = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))
    {
        let path_start = rest.find('/')?;
        &rest[path_start..]
    } else {
        return None;
    };

    Some(path.split_once('?').map_or(path, |(path, _)| path))
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_url_uses_configured_host_and_bound_port() {
        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::Local,
            ..ResourcePack::default()
        };
        config.download_host = "example.org".to_string();
        let local = LocalResourcePack {
            route: resource_pack_route(config.id),
            bind: "127.0.0.1:0".parse().unwrap(),
            public_port: 25566,
            hash: String::new(),
            bytes: std::sync::Arc::new(Vec::new()),
        };

        assert_eq!(
            local_download_url(&config, "ignored", &local),
            format!("http://example.org:25566/resource-pack/{}.zip", config.id)
        );
    }

    #[test]
    fn public_download_url_keeps_scheme_and_omits_bound_port() {
        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::Local,
            ..ResourcePack::default()
        };
        config.download_host = "https://cdn.example.org/packs".to_string();
        let local = LocalResourcePack {
            route: resource_pack_route(config.id),
            bind: "127.0.0.1:0".parse().unwrap(),
            public_port: 25566,
            hash: String::new(),
            bytes: std::sync::Arc::new(Vec::new()),
        };

        assert_eq!(
            local_download_url(&config, "ignored", &local),
            format!(
                "https://cdn.example.org/packs/resource-pack/{}.zip",
                config.id
            )
        );
    }

    #[test]
    fn object_storage_url_prefers_public_base_url_for_cdn_and_edgeone() {
        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::ObjectStorage,
            ..ResourcePack::default()
        };
        config.object_storage.public_base_url = "https://packs.example.com/cache/".to_string();
        config.object_storage.object_key = "/minecraft/server.zip".to_string();

        assert_eq!(
            object_storage_download_url(&config).as_deref(),
            Some("https://packs.example.com/cache/minecraft/server.zip")
        );
    }

    #[test]
    fn object_storage_url_builds_virtual_host_endpoint() {
        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::ObjectStorage,
            ..ResourcePack::default()
        };
        config.object_storage.endpoint = "obs.cn-north-4.myhuaweicloud.com".to_string();
        config.object_storage.bucket = "qexed-pack".to_string();
        config.object_storage.object_key = "resourcepacks/server.zip".to_string();

        assert_eq!(
            object_storage_download_url(&config).as_deref(),
            Some("https://qexed-pack.obs.cn-north-4.myhuaweicloud.com/resourcepacks/server.zip")
        );
    }

    #[test]
    fn object_storage_url_builds_path_style_endpoint() {
        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::ObjectStorage,
            ..ResourcePack::default()
        };
        config.object_storage.endpoint = "https://cos.ap-guangzhou.myqcloud.com".to_string();
        config.object_storage.bucket = "qexed-1250000000".to_string();
        config.object_storage.object_key = "resourcepacks/server.zip".to_string();
        config.object_storage.force_path_style = true;

        assert_eq!(
            object_storage_download_url(&config).as_deref(),
            Some("https://cos.ap-guangzhou.myqcloud.com/qexed-1250000000/resourcepacks/server.zip")
        );
    }

    #[test]
    fn request_path_accepts_origin_and_absolute_form_targets() {
        let request = "GET /resource-pack/test.zip HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(request_path(request), Some("/resource-pack/test.zip"));

        let request = "GET http://127.0.0.1:25566/resource-pack/test.zip?cache=1 HTTP/1.1\r\n\r\n";
        assert_eq!(request_path(request), Some("/resource-pack/test.zip"));
    }

    #[tokio::test]
    async fn local_server_returns_pack_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("server.zip");
        tokio::fs::write(&pack_path, b"zip-bytes").await.unwrap();

        let mut config = ResourcePack {
            enable: true,
            source: ResourcePackSource::Local,
            ..ResourcePack::default()
        };
        config.path = pack_path.to_string_lossy().to_string();
        config.download_bind = "127.0.0.1:0".to_string();

        let mut manager = ResourcePackManager::from_config(&config).await.unwrap();
        manager.start().await.unwrap();
        let offer = manager.offer(&config, "127.0.0.1").unwrap();

        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = http.get(&offer.url).send().await.unwrap();
        let status = response.status();
        let body = response.bytes().await.unwrap();
        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(body, "zip-bytes");
        assert_eq!(offer.hash, sha1_hex(b"zip-bytes"));
    }
}
