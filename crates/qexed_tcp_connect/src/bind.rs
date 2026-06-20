use std::{fmt::Debug, io, str::FromStr};
use tokio::net::TcpListener;

#[derive(Debug, qexed_error_macros::I18nErrorDisplay)]
pub enum BindError {
    #[error("qexed_tcp_connect.addr_parse_error",addr=field_0)]
    AddrParseError(String),
    #[error("qexed_tcp_connect.invalid_input")]
    InvalidInput,
    #[error("qexed_tcp_connect.addr_in_use",addr = field_0.to_string())]
    AddrInUse(std::net::SocketAddr),
    #[error("qexed_tcp_connect.permission_denied")]
    PermissionDenied,
    #[error("qexed_tcp_connect.addr_not_available",addr = field_0.to_string(),ip = field_0.ip().to_string())]
    AddrNotAvailable(std::net::SocketAddr),
    #[error("qexed_tcp_connect.network_unreachable",addr = field_0.to_string(),ip = field_0.ip().to_string())]
    NetworkUnreachable(std::net::SocketAddr),
    #[error("qexed_tcp_connect.connection_refused",addr = field_0.to_string())]
    ConnectionRefused(std::net::SocketAddr),
    #[error("qexed_tcp_connect.timed_out",addr = field_0.to_string())]
    TimedOut(std::net::SocketAddr),
    #[error("qexed_tcp_connect.other",error = field_0.to_string())]
    Other(io::Error),
}
impl std::error::Error for BindError {}
pub async fn bind(addr: &str) -> Result<TcpListener, BindError> {
    let socket_addr = match std::net::SocketAddr::from_str(addr) {
        Ok(v) => v,
        Err(_) => {
            return Err(BindError::AddrParseError(addr.to_string()));
        }
    };

    match TcpListener::bind(socket_addr).await {
        Ok(listener) => Ok(listener),
        Err(e) => Err(match e.kind() {
            io::ErrorKind::AddrInUse => BindError::AddrInUse(socket_addr),
            io::ErrorKind::PermissionDenied => BindError::PermissionDenied,
            io::ErrorKind::AddrNotAvailable => BindError::AddrNotAvailable(socket_addr),
            io::ErrorKind::InvalidInput => BindError::InvalidInput,
            io::ErrorKind::NetworkUnreachable => BindError::NetworkUnreachable(socket_addr),
            io::ErrorKind::ConnectionRefused => BindError::ConnectionRefused(socket_addr),
            io::ErrorKind::TimedOut => BindError::TimedOut(socket_addr),
            _ => BindError::Other(e),
        }),
    }
}
