use bytes::BytesMut;
use qexed_tcp_connect::PacketRead;
use tokio::io::{AsyncReadExt, AsyncBufReadExt, BufReader};
use std::net::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr};
// impl super::LogicTask {
//     pub async fn haproxy_get_client_ip(
//         &self,
//         stream: &mut tokio::net::TcpStream,
//     ) -> anyhow::Result<Option<std::net::SocketAddr>> {
//         HAProxyV2::new(stream).get_client_ip()
//     }
// }

// pub struct HAProxyV2<'a>{
//     stream: &'a mut tokio::net::TcpStream,
// }
// impl HAProxyV2<'a> {
//     pub fn new(
//         stream: &'a mut tokio::net::TcpStream,
//     )->Self{
//         Self { stream }
//     }
//     pub fn get_client_ip(&self)->anyhow::Result<Option<std::net::SocketAddr>>{
//         let mut buf = vec![0; 12];
//         self.stream.read_exact(&mut buf);
//         log::info!("buffer:{:?}",buf);
//         Err(anyhow::anyhow!("开发中,暂未完成"))
//     }
// }