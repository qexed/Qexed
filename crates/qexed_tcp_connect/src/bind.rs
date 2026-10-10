use std::str::FromStr;

use tokio::net::TcpListener;

use crate::error::TcpConnectError;

pub async fn bind(addr: &str) -> Result<TcpListener, TcpConnectError> {
    let socket_addr = std::net::SocketAddr::from_str(addr)?;

    Ok(TcpListener::bind(socket_addr).await?)
}
