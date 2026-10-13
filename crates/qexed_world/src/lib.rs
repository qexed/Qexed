//! 区块行为 crate：世界数据的协议侧表达。
//!
//! 职责（对应 Pumpkin 的 `world/chunker.rs` + `net/chunk_sender.rs`）：
//! 1. 空区块 / 超平区块的协议字节构造（[chunk::empty_chunk_packet]）
//! 2. 真实存档区块编码（[anvil_chunk::ChunkPipeline]）：
//!    anvil 3×3 邻域 → qexed_light 光照计算 → 26.1.2 section/palette/heightmap 编码
//! 3. 出生区区块批次计划（[stream::SpawnChunkStreamer]）：
//!    ChunkBatchStart → N × ChunkData → ChunkBatchFinished
//! 4. 远程地形来源（[tcp_source::TcpChunkSource]）：qexed 作为 TCP 客户端向
//!    原版区块召唤服务请求地形（存档命中读盘 / 未命中生成），回包光照由
//!    [tcp_light] 经 qexed_light 重算后覆写
//!
//! 动态批次调速（客户端 ChunkBatchReceived 回报 desired_chunks_per_tick）留待后续。
//!
//! 依赖方向（MIGRATION-PLAN）：
//! `qexed_play → qexed_world → {qexed_protocol, qexed_registry, qexed_light, qexed_anvil}`。

pub mod anvil_chunk;
pub mod blocks;
pub mod chunk;
pub mod error;
pub mod light;
pub mod stream;
pub mod tcp_light;
pub mod tcp_source;

pub use error::{Result, WorldError};
