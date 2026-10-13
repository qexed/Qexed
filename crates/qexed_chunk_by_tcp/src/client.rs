//! TCP 客户端实现：帧层复用 qexed_tcp_connect，包体手工解析
//! （协议结构体只实现 Packet 而非 PacketCodec，且包体极小，手工解析更直接）。

use bytes::{Bytes, BytesMut};
use qexed_packet::net_types::RestBuffer;
use qexed_protocol::to_server::play::custom_payload::CustomPayload as ServerboundPayload;
use qexed_protocol::to_server::play::keep_alive::KeepAlive as ServerboundKeepAlive;
use qexed_tcp_connect::{PacketSink, PacketStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

/// 默认服务端口。
pub const DEFAULT_PORT: u16 = 54132;
/// 区块请求通道（serverbound custom_payload）。
pub const CHUNK_SUMMON_CHANNEL: &str = "qexed:chunk_summon";
/// 区块数据回包通道（clientbound custom_payload）。
pub const CHUNK_DATA_CHANNEL: &str = "qexed:chunk_data";

/// 请求的维度（服务端当前仅实现主世界；该字节为协议预留）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Overworld = 0,
    Nether = 1,
    End = 2,
}

/// 客户端错误。
#[derive(Debug, thiserror::Error)]
pub enum ChunkClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tcp connect layer: {0}")]
    Tcp(#[from] qexed_tcp_connect::error::TcpConnectError),
    #[error("packet decode failed: {0}")]
    Decode(String),
    #[error("connection closed by server")]
    ConnectionClosed,
    #[error("server sent disconnect")]
    Disconnected,
    #[error("unexpected packet id 0x{0:0>2x}")]
    UnexpectedPacket(u32),
    #[error("chunk payload too short ({0} bytes, need >= 9)")]
    PayloadTooShort(usize),
    #[error("chunk body varint length out of range")]
    VarintRange,
    #[error("chunk body truncated: declared {declared} bytes, got {actual}")]
    BodyTruncated { declared: usize, actual: usize },
    #[error("chunk ({got_x},{got_z}) does not match requested ({want_x},{want_z})")]
    ChunkMismatch { got_x: i32, got_z: i32, want_x: i32, want_z: i32 },
    #[error("expected chunk_data payload, got channel {0:?}")]
    UnexpectedChannel(String),
}

/// 一个区块：坐标 + 完整 ClientboundLevelChunkWithLightPacket 包体（不含包 ID）。
#[derive(Debug, Clone)]
pub struct ChunkData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub packet: Bytes,
}

impl ChunkData {
    /// 包体字节数（即服务端编码的区块包大小，e2e 断言用）。
    pub fn len(&self) -> usize {
        self.packet.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packet.is_empty()
    }
}

/// 到 chunk-summon 服务的单个连接（帧格式：VarInt 长度前缀，无压缩/加密）。
pub struct ChunkTcpClient {
    sink: PacketSink<OwnedWriteHalf>,
    stream: PacketStream<OwnedReadHalf>,
}

/// clientbound play 包 ID（bundle 偏移后的 qexed_protocol 约定值）。
mod cb {
    pub const CHUNK_BATCH_FINISHED: i32 = 0x0b;
    pub const CHUNK_BATCH_START: i32 = 0x0c;
    pub const DISCONNECT: i32 = 0x20;
    pub const CUSTOM_PAYLOAD: i32 = 0x18;
    pub const KEEP_ALIVE: i32 = 0x2c;
    pub const PING: i32 = 0x3d;
}

impl ChunkTcpClient {
    /// 连接 chunk-summon 服务（无登录/配置阶段，直接进入 play 式包交换）。
    pub async fn connect(addr: impl tokio::net::ToSocketAddrs) -> Result<Self, ChunkClientError> {
        let socket = TcpStream::connect(addr).await?;
        let (read_half, write_half) = socket.into_split();
        Ok(Self {
            sink: PacketSink::new(write_half),
            stream: PacketStream::new(read_half),
        })
    }

    /// 连接本机默认端口 54132。
    pub async fn connect_local() -> Result<Self, ChunkClientError> {
        Self::connect(("127.0.0.1", DEFAULT_PORT)).await
    }

    /// 读下一包并按 ID 分类；返回自定义载荷的 (通道, 数据)。
    /// 断开/异常包返回 Err；batch/ping 等可忽略包在内部跳过。
    async fn next_payload(
        &mut self,
    ) -> Result<(String, Vec<u8>), ChunkClientError> {
        loop {
            let Some(payload) = self.stream.read_packet().await? else {
                return Err(ChunkClientError::ConnectionClosed);
            };
            let mut buf = &payload[..];
            let id = read_varint(&mut buf).ok_or(ChunkClientError::Decode("packet id".into()))? as i32;
            match id {
                cb::CUSTOM_PAYLOAD => {
                    let channel = read_string(&mut buf).ok_or(ChunkClientError::Decode("channel".into()))?;
                    return Ok((channel, buf.to_vec()));
                }
                cb::KEEP_ALIVE => {
                    if buf.len() < 8 {
                        return Err(ChunkClientError::Decode("keep_alive truncated".into()));
                    }
                    // 回显探测：调用方通过 ping() 等待；此处跳过由 ping 自行读取
                    return Err(ChunkClientError::Decode("unexpected keep_alive".into()));
                }
                cb::DISCONNECT => return Err(ChunkClientError::Disconnected),
                cb::CHUNK_BATCH_START | cb::CHUNK_BATCH_FINISHED | cb::PING => continue,
                other => return Err(ChunkClientError::UnexpectedPacket(other as u32)),
            }
        }
    }

    /// keep-alive 探测：发送 0x1c，等待原值回显 0x2c。
    pub async fn ping(&mut self, id: i64) -> Result<i64, ChunkClientError> {
        self.sink.send(ServerboundKeepAlive { keep_alive_id: id }).await?;
        loop {
            let Some(payload) = self.stream.read_packet().await? else {
                return Err(ChunkClientError::ConnectionClosed);
            };
            let mut buf = &payload[..];
            let pid = read_varint(&mut buf).ok_or(ChunkClientError::Decode("packet id".into()))? as i32;
            match pid {
                cb::KEEP_ALIVE => {
                    if buf.len() < 8 {
                        return Err(ChunkClientError::Decode("keep_alive truncated".into()));
                    }
                    return Ok(i64::from_be_bytes(buf[..8].try_into().unwrap()));
                }
                cb::CUSTOM_PAYLOAD => continue, // 无关回显：忽略
                cb::DISCONNECT => return Err(ChunkClientError::Disconnected),
                cb::CHUNK_BATCH_START | cb::CHUNK_BATCH_FINISHED | cb::PING => continue,
                other => return Err(ChunkClientError::UnexpectedPacket(other as u32)),
            }
        }
    }

    /// 请求一个区块并等待对应回包（校验坐标一致）。
    pub async fn request_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        dimension: Dimension,
    ) -> Result<ChunkData, ChunkClientError> {
        self.send_chunk_request(chunk_x, chunk_z, dimension).await?;
        let mut pending = vec![(chunk_x, chunk_z)];
        let mut out = self.collect_chunk_replies(&mut pending).await?;
        out.pop()
            .ok_or_else(|| ChunkClientError::Decode("collect_chunk_replies 空返回".into()))
    }

    /// 发送一个区块请求（包体：i32 x + i32 z + u8 维度）。
    async fn send_chunk_request(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        dimension: Dimension,
    ) -> Result<(), ChunkClientError> {
        let mut body = BytesMut::new();
        body.extend_from_slice(&chunk_x.to_be_bytes());
        body.extend_from_slice(&chunk_z.to_be_bytes());
        body.extend_from_slice(&[dimension as u8]);
        self.sink
            .send(ServerboundPayload {
                channel: CHUNK_SUMMON_CHANNEL.to_string(),
                data: RestBuffer(body.to_vec()),
            })
            .await?;
        Ok(())
    }

    /// 流水线批量请求：先发全部请求再统一收包，网络往返从 N 次降到
    /// 一次发送窗口（服务端按序回包，按回包坐标匹配）。
    ///
    /// 回包坐标不在请求集 → ChunkMismatch；重复回包 → Decode 错误。
    pub async fn request_chunks(
        &mut self,
        coords: &[(i32, i32)],
        dimension: Dimension,
    ) -> Result<Vec<ChunkData>, ChunkClientError> {
        for &(chunk_x, chunk_z) in coords {
            self.send_chunk_request(chunk_x, chunk_z, dimension).await?;
        }
        let mut pending = coords.to_vec();
        self.collect_chunk_replies(&mut pending).await
    }

    /// 收包直到 pending 中全部坐标到齐；按坐标移除并按到达序返回。
    async fn collect_chunk_replies(
        &mut self,
        pending: &mut Vec<(i32, i32)>,
    ) -> Result<Vec<ChunkData>, ChunkClientError> {
        let mut out = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let (channel, data) = self.next_payload().await?;
            if channel != CHUNK_DATA_CHANNEL {
                continue; // 其他通道回显：忽略
            }
            let chunk = parse_chunk_data(data)?;
            let key = (chunk.chunk_x, chunk.chunk_z);
            let Some(pos) = pending.iter().position(|c| *c == key) else {
                return Err(ChunkClientError::ChunkMismatch {
                    got_x: chunk.chunk_x,
                    got_z: chunk.chunk_z,
                    want_x: pending
                        .first()
                        .map(|c| c.0)
                        .unwrap_or(i32::MAX),
                    want_z: pending
                        .first()
                        .map(|c| c.1)
                        .unwrap_or(i32::MAX),
                });
            };
            pending.remove(pos);
            out.push(chunk);
        }
        Ok(out)
    }
}

/// 解析 chunk_data 载荷：外层 VarInt(body 长度) + i32 x + i32 z + 区块包字节。
/// （外层长度是服务端 writeByteArray 的长度前缀 = 8 + 包长；其后不再有内层长度，
/// 区块包体就是余下全部字节。）
fn parse_chunk_data(data: Vec<u8>) -> Result<ChunkData, ChunkClientError> {
    let mut buf = &data[..];
    let body_len = read_varint(&mut buf).ok_or(ChunkClientError::VarintRange)? as usize;
    if buf.len() < body_len || buf.len() < 8 {
        return Err(ChunkClientError::PayloadTooShort(buf.len()));
    }
    let chunk_x = i32::from_be_bytes(buf[0..4].try_into().unwrap());
    let chunk_z = i32::from_be_bytes(buf[4..8].try_into().unwrap());
    let packet = &buf[8..body_len];
    Ok(ChunkData {
        chunk_x,
        chunk_z,
        packet: Bytes::copy_from_slice(packet),
    })
}

/// VarInt：最多 5 字节，返回 (值, 消耗字节数)。
fn read_varint(buf: &mut &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    let mut shift = 0;
    loop {
        let &b = buf.first()?;
        *buf = &buf[1..];
        value |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
}

/// MC 字符串：VarInt 长度 + UTF-8。
fn read_string(buf: &mut &[u8]) -> Option<String> {
    let len = read_varint(buf)? as usize;
    if buf.len() < len {
        return None;
    }
    let s = String::from_utf8(buf[..len].to_vec()).ok()?;
    *buf = &buf[len..];
    Some(s)
}
