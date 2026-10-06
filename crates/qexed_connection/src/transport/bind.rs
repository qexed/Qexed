//! TCP 监听绑定（v4 qexed_tcp_connect::bind 迁移）。
//!
//! v4 用 qexed_error_macros::I18nErrorDisplay + rust_i18n，v6 不存在，
//! 改为 thiserror + qexed_language::t（Display 查翻译表）。
//! v4 BindError 的变体与语义全部保留（字段化以便 thiserror 具名插值）。

use std::{io, str::FromStr};
use tokio::net::TcpListener;

use crate::error::ConnectionError;

/// 绑定监听地址失败。
#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error("{}", t("qexed.connection.bind.addr_parse_error").replace("%{addr}", addr))]
    AddrParseError { addr: String },
    #[error("{}", t("qexed.connection.bind.invalid_input"))]
    InvalidInput,
    #[error("{}", t("qexed.connection.bind.addr_in_use").replace("%{addr}", &addr.to_string()))]
    AddrInUse { addr: std::net::SocketAddr },
    #[error("{}", t("qexed.connection.bind.permission_denied"))]
    PermissionDenied,
    #[error("{}", t("qexed.connection.bind.addr_not_available").replace("%{addr}", &addr.to_string()).replace("%{ip}", ip))]
    AddrNotAvailable { addr: std::net::SocketAddr, ip: String },
    #[error("{}", t("qexed.connection.bind.network_unreachable").replace("%{addr}", &addr.to_string()).replace("%{ip}", ip))]
    NetworkUnreachable { addr: std::net::SocketAddr, ip: String },
    #[error("{}", t("qexed.connection.bind.connection_refused").replace("%{addr}", &addr.to_string()))]
    ConnectionRefused { addr: std::net::SocketAddr },
    #[error("{}", t("qexed.connection.bind.timed_out").replace("%{addr}", &addr.to_string()))]
    TimedOut { addr: std::net::SocketAddr },
    #[error("{}", t("qexed.connection.bind.other").replace("%{error}", &error.to_string()))]
    Other { error: io::Error },
}

/// 翻译查询（本文件内简名，避免每处写全路径）。
fn t(key: &str) -> String {
    qexed_language::t(key)
}

/// 绑定 TCP 监听（v4 qexed_tcp_connect::bind 同名迁移）。
pub async fn bind(addr: &str) -> Result<TcpListener, ConnectionError> {
    let Ok(socket_addr) = std::net::SocketAddr::from_str(addr) else {
        return Err(BindError::AddrParseError { addr: addr.to_string() }.into());
    };

    match TcpListener::bind(socket_addr).await {
        Ok(listener) => Ok(listener),
        Err(e) => Err(match e.kind() {
            io::ErrorKind::AddrInUse => BindError::AddrInUse { addr: socket_addr },
            io::ErrorKind::PermissionDenied => BindError::PermissionDenied,
            io::ErrorKind::AddrNotAvailable => BindError::AddrNotAvailable {
                addr: socket_addr,
                ip: socket_addr.ip().to_string(),
            },
            io::ErrorKind::InvalidInput => BindError::InvalidInput,
            io::ErrorKind::NetworkUnreachable => BindError::NetworkUnreachable {
                addr: socket_addr,
                ip: socket_addr.ip().to_string(),
            },
            io::ErrorKind::ConnectionRefused => BindError::ConnectionRefused { addr: socket_addr },
            io::ErrorKind::TimedOut => BindError::TimedOut { addr: socket_addr },
            _ => BindError::Other { error: e },
        }
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn binds_ephemeral_loopback_port() {
        let listener = bind("127.0.0.1:0").await.unwrap();
        assert!(listener.local_addr().is_ok());
    }

    #[tokio::test]
    async fn rejects_invalid_address() {
        let err = bind("not-an-addr").await.unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::Bind(BindError::AddrParseError { .. })
        ));
    }
}
