//! 资源包推送源（v4 connection::configuration 的 ResourcePackManager 依赖拆分版）。
//!
//! v4 的 ResourcePackManager 位于 qexed 本体（单 crate 无依赖方向问题）；v6 拆分
//! crate 后 qexed_server 不能反向依赖 qexed_connection（server 被更上层依赖），
//! 因此 Local 来源的"读 zip → sha1 → 起本地 HTTP 下载服务"整体下沉到连接域
//! （协议握手与下载服务都归连接域，依赖方向 connection → 无外部域依赖），
//! qexed_server 若需要同一能力，经 ResourcePackOfferFn 回调注入此处实现即可。

use std::sync::Arc;

use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use crate::config::{ResourcePack, ResourcePackSource};
use crate::error::{ConnectionError, Result};

/// 一次资源包推送要发给客户端的下载参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourcePackOffer {
    pub url: String,
    pub hash: String,
}

/// 按配置解析推送参数：
/// - Url：直接使用配置 url（为空告警跳过，与 v4 行为一致）；
/// - ObjectStorage：按 endpoint/bucket/object_key 拼直链（v4 只做 URL 拼接，不上传）；
/// - Local：读取 zip → sha1 → 起本地 HTTP 下载服务 → 生成下载 URL。
pub(crate) async fn resolve_offer(config: &ResourcePack, login_host: &str) -> Result<Option<ResourcePackOffer>> {
    if !config.enable {
        return Ok(None);
    }

    match config.source {
        ResourcePackSource::Url => {
            let url = config.url.trim();
            if url.is_empty() {
                log::warn!("{}", qexed_language::t("qexed.connection.config.resource_pack_no_url"));
                return Ok(None);
            }
            Ok(Some(ResourcePackOffer {
                url: url.to_string(),
                hash: config.hash.trim().to_string(),
            }))
        }
        ResourcePackSource::ObjectStorage => Ok(object_storage_download_url(config).map(|url| ResourcePackOffer {
            url,
            hash: config.hash.trim().to_string(),
        })),
        ResourcePackSource::Local => {
            let mut local = LocalResourcePack::from_config(config).await?;
            local.start().await?;
            Ok(Some(ResourcePackOffer {
                url: local_download_url(config, login_host, &local),
                hash: local.hash,
            }))
        }
    }
}

/// 本地资源包：文件字节缓存 + 独立端口的极简 HTTP 下载服务（v4 local.rs 迁移）。
pub(crate) struct LocalResourcePack {
    route: String,
    bind: std::net::SocketAddr,
    public_port: u16,
    hash: String,
    bytes: Arc<Vec<u8>>,
}

impl LocalResourcePack {
    pub(crate) async fn from_config(config: &ResourcePack) -> Result<Self> {
        let path = std::path::Path::new(config.path.trim());
        let bytes = tokio::fs::read(path).await.map_err(|err| {
            ConnectionError::msg(qexed_language::t("qexed.connection.resource_pack.local_read_failed")
                .replace("%{path}", &path.display().to_string())
                .replace("%{error}", &err.to_string()))
        })?;
        let hash = if config.hash.trim().is_empty() {
            sha1_hex(&bytes)
        } else {
            config.hash.trim().to_string()
        };
        let bind = bind_download_addr(&config.download_bind)?;
        let route = resource_pack_route(config.pack_id());

        Ok(Self {
            route,
            bind,
            public_port: bind.port(),
            hash,
            bytes: Arc::new(bytes),
        })
    }

    /// 绑定下载端口并启动 accept 循环（bind 指定端口 0 时以实际端口对外）。
    pub(crate) async fn start(&mut self) -> Result<()> {
        let listener = TcpListener::bind(self.bind).await.map_err(|err| {
            ConnectionError::msg(qexed_language::t("qexed.connection.resource_pack.bind_failed")
                .replace("%{addr}", &self.bind.to_string())
                .replace("%{error}", &err.to_string()))
        })?;
        self.public_port = listener.local_addr()?.port();
        let served = ServedPack {
            route: self.route.clone(),
            bytes: self.bytes.clone(),
        };

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let served = served.clone();
                        tokio::spawn(async move {
                            if let Err(err) = handle_download(stream, served).await {
                                log::debug!(
                                    "{}",
                                    qexed_language::t("qexed.connection.resource_pack.download_failed")
                                        .replace("%{error}", &err.to_string())
                                );
                            }
                        });
                    }
                    Err(err) => {
                        log::warn!(
                            "{}",
                            qexed_language::t("qexed.connection.resource_pack.accept_failed")
                                .replace("%{error}", &err.to_string())
                        );
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}

/// 下载服务持久的共享载荷。
#[derive(Clone)]
struct ServedPack {
    route: String,
    bytes: Arc<Vec<u8>>,
}

async fn handle_download(mut stream: tokio::net::TcpStream, served: ServedPack) -> Result<()> {
    let request_bytes = read_http_headers(&mut stream).await?;
    let request = std::str::from_utf8(&request_bytes).unwrap_or_default();
    let path = request_path(request);

    if path == Some(served.route.as_str()) {
        write_response(&mut stream, 200, "OK", "application/zip", &served.bytes).await
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

/// HTTP 头结束符（常量形式，避免转义字面量在各编辑器/工具间被改写）。
const CRLF_CRLF: &[u8] = b"\r\n\r\n";
const LF_LF: &[u8] = b"\n\n";

/// 极简 HTTP/1.1 请求头读取（读到空行或 8 KiB 上限）。
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
        if request.windows(4).any(|window| window == CRLF_CRLF)
            || request.windows(2).any(|window| window == LF_LF)
            || request.len() >= MAX_HEADER_SIZE
        {
            break;
        }
    }

    Ok(request)
}

/// 从请求行取路径（支持 origin-form 与 absolute-form，query 丢弃）。
pub(crate) fn request_path(request: &str) -> Option<&str> {
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

fn bind_download_addr(bind: &str) -> Result<std::net::SocketAddr> {
    bind.trim()
        .parse::<std::net::SocketAddr>()
        .map_err(|err| {
        ConnectionError::msg(qexed_language::t("qexed.connection.resource_pack.bind_invalid")
            .replace("%{addr}", bind)
            .replace("%{error}", &err.to_string()))
    })
}

pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// 下载路由：/resource-pack/<pack id>.zip（v4 同款，客户端只依赖 URL 可达）。
pub(crate) fn resource_pack_route(id: uuid::Uuid) -> String {
    format!("/resource-pack/{id}.zip")
}

/// 拼下载 URL：download_host 为完整 base 时直接接路由；否则用 login_host 兜底。
pub(crate) fn local_download_url(config: &ResourcePack, login_host: &str, local: &LocalResourcePack) -> String {
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

fn join_url_path(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// 对象存储直链（v4 object_storage.rs 迁移：public_base_url 优先，其次 endpoint+bucket 拼接）。
pub(crate) fn object_storage_download_url(config: &ResourcePack) -> Option<String> {
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

