//! 通过 TCP 直连 chunk-summon 服务拉取原版区块的客户端库。
//!
//! 服务端（tools/mc-vanilla-chunk-summon 的 runServer 任务）在 54132 端口上
//! 以原版帧格式（VarInt 长度前缀，无压缩无加密、无登录/配置阶段）交换 play 包：
//!
//! - 请求：serverbound custom_payload(0x16)，通道 qexed:chunk_summon，
//!   载荷 = VarInt 通道串 + i32 chunkX + i32 chunkZ + u8 维度
//! - 响应：clientbound custom_payload(0x18)，通道 qexed:chunk_data，
//!   载荷 = VarInt(body 长度) + body；body = i32 chunkX + i32 chunkZ
//!   + VarInt(len) + 完整区块包体
//! - keep_alive(0x1c→0x2c) 原值回显；其他通道原样回显
//! - disconnect(0x20)：服务端出错时发送（reason 为 NBT 文本）
//!
//! # 示例
//!
//! ```no_run
//! use qexed_chunk_by_tcp::{ChunkTcpClient, Dimension};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ChunkTcpClient::connect_local().await?;
//! let chunk = client.request_chunk(0, 0, Dimension::Overworld).await?;
//! assert_eq!((chunk.chunk_x, chunk.chunk_z), (0, 0));
//! # Ok(())
//! # }
//! ```

mod client;
mod pooled;

pub use client::{
    ChunkClientError, ChunkData, ChunkTcpClient, Dimension, CHUNK_DATA_CHANNEL,
    CHUNK_SUMMON_CHANNEL, DEFAULT_PORT,
};
pub use pooled::PooledChunkClient;
