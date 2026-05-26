use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::url::resource_pack_route;

#[derive(Clone)]
pub(super) struct LocalResourcePack {
    pub(super) route: String,
    pub(super) bind: std::net::SocketAddr,
    pub(super) public_port: u16,
    pub(super) hash: String,
    pub(super) bytes: std::sync::Arc<Vec<u8>>,
}

impl LocalResourcePack {
    pub(super) async fn from_config(
        config: &qexed_config::app::qexed::server::ResourcePack,
    ) -> Result<Self> {
        let path = std::path::Path::new(config.path.trim());
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("无法读取本地资源包文件: {}", path.display()))?;
        let hash = if config.hash.trim().is_empty() {
            sha1_hex(&bytes)
        } else {
            config.hash.trim().to_string()
        };
        let bind = bind_download_addr(&config.download_bind)?;
        let route = resource_pack_route(config.id);

        Ok(Self {
            route,
            bind,
            public_port: bind.port(),
            hash,
            bytes: std::sync::Arc::new(bytes),
        })
    }

    pub(super) async fn start(&mut self) -> Result<()> {
        let listener = TcpListener::bind(self.bind)
            .await
            .with_context(|| format!("无法监听资源包下载地址 {}", self.bind))?;
        self.public_port = listener.local_addr()?.port();
        let served = self.clone();

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
}

fn bind_download_addr(bind: &str) -> Result<std::net::SocketAddr> {
    bind.trim()
        .parse()
        .with_context(|| format!("资源包下载监听地址无效: {bind}"))
}

pub(super) fn sha1_hex(bytes: &[u8]) -> String {
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

pub(super) fn request_path(request: &str) -> Option<&str> {
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
