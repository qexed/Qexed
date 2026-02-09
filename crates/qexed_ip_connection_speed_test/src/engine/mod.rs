pub mod pika;
pub mod simple;
#[async_trait::async_trait]
pub trait Engine{
    async fn check_ip(&mut self, socket_addr: std::net::SocketAddr)->anyhow::Result<()>;
}
#[derive(Default)]
pub struct NoEngine {}
#[async_trait::async_trait]
impl Engine for NoEngine {
    async fn check_ip(&mut self, _socket_addr: std::net::SocketAddr)->anyhow::Result<()> {
        Err(anyhow::anyhow!("未启用服务的情况下禁止调用此接口"))
    }
}
