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
            ResourcePackState::Local(local) => Some(ResourcePackOffer {
                url: local_download_url(config, login_host, local),
                hash: local.hash.clone(),
            }),
        }
    }
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
    let mut buffer = [0_u8; 2048];
    let size = stream.read(&mut buffer).await?;
    let request = std::str::from_utf8(&buffer[..size]).unwrap_or_default();
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

fn request_path(request: &str) -> Option<&str> {
    let request_line = request.lines().next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    if method == "GET" { Some(path) } else { None }
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

        let response = reqwest::get(&offer.url).await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.bytes().await.unwrap(), "zip-bytes");
        assert_eq!(offer.hash, sha1_hex(b"zip-bytes"));
    }
}
