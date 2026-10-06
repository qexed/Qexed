//! 集群 RPC：分片（shard）与主服务器之间的长度前缀帧协议。

// 公共类型已上移 qexed_protocol::types（避免与 qexed_world 循环依赖）
pub use qexed_protocol::types::{ClusterCollectedItem, ClusterEntityRendering, ClusterEntitySpawning, ClusterLightAlgorithm, ClusterPacketBatch, ClusterPlayerSnapshot, ClusterRequest, ClusterResponse};

// 迁移自 v4 crates/qexed/src/cluster_rpc.rs。
// 适配差异：v4 用 postcard 序列化帧负载；v6 workspace 无 postcard，
// 改用 serde_json（帧格式：4 字节大端长度 + UTF-8 JSON）。
// 帧大小上限与超时常量保持 v4 语义。

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_millis(1500);
pub const SLOW_RESPONSE_THRESHOLD: Duration = Duration::from_millis(100);
pub const FAILURE_COOLDOWN: Duration = Duration::from_secs(2);

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;










/// 实体渲染距离参数（v4 qexed_config::app::qexed::server::EntityRendering 的 RPC 投影）。

/// 实体生成参数（v4 qexed_config::app::qexed::server::EntitySpawning 的 RPC 投影）。



/// 发送一次集群 RPC 请求（连接超时 + 读写超时各参数控制）。
pub fn send_request(
    endpoint: &str,
    request: &ClusterRequest,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<ClusterResponse> {
    let addr = endpoint_addr(endpoint)?;
    let mut stream = TcpStream::connect_timeout(&addr, connect_timeout)
        .map_err(|source| ServerError::IoContext {
            context: format!("cluster shard connect failed: endpoint={endpoint}"),
            source,
        })?;
    stream.set_read_timeout(Some(request_timeout))?;
    stream.set_write_timeout(Some(request_timeout))?;
    write_frame(&mut stream, request)?;
    read_frame(&mut stream)
}

/// 写一帧：4 字节大端长度 + JSON 负载。
pub fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ServerError::ClusterFrameTooLarge(payload.len()));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| ServerError::ClusterFrameTooLarge(payload.len()))?
        .to_be_bytes();
    writer.write_all(&len)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// 读一帧（与 write_frame 对偶）。
pub fn read_frame<T: for<'de> Deserialize<'de>>(reader: &mut impl Read) -> Result<T> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(ServerError::ClusterFrameTooLarge(len));
    }
    let mut payload = vec![0; len];
    reader.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

/// 解析 tcp://host:port 形式的集群 endpoint。
pub fn endpoint_addr(endpoint: &str) -> Result<SocketAddr> {
    let raw = endpoint
        .strip_prefix("tcp://")
        .ok_or_else(|| ServerError::InvalidEndpoint(endpoint.to_string()))?;
    raw.to_socket_addrs()?
        .next()
        .ok_or_else(|| ServerError::InvalidEndpoint(format!("cluster endpoint did not resolve: {endpoint}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> ClusterRequest {
        ClusterRequest::BlockStateAt {
            dimension: "minecraft:overworld".to_string(),
            x: 1,
            y: 64,
            z: -2,
        }
    }

    #[test]
    fn frame_roundtrip() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, &sample_request()).unwrap();
        let mut cursor = buffer.as_slice();
        let decoded: ClusterRequest = read_frame(&mut cursor).unwrap();
        assert_eq!(decoded, sample_request());
    }

    #[test]
    fn oversized_frame_is_rejected() {
        // 手工构造超过上限的长度前缀
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend_from_slice(&u32::try_from(MAX_FRAME_BYTES + 1).unwrap().to_be_bytes());
        buffer.extend_from_slice(&[0_u8; 8]);
        let mut cursor = buffer.as_slice();
        let result: Result<ClusterRequest> = read_frame(&mut cursor);
        assert!(matches!(
            result,
            Err(ServerError::ClusterFrameTooLarge(size)) if size == MAX_FRAME_BYTES + 1
        ));
    }

    #[test]
    fn endpoint_addr_parses_tcp_urls() {
        let addr = endpoint_addr("tcp://127.0.0.1:25599").unwrap();
        assert_eq!(addr.port(), 25599);
        assert!(endpoint_addr("http://127.0.0.1:1").is_err());
    }
}
