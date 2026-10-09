//! qexed_anvil：Minecraft Java 版 Anvil 世界存档（.mca 区域文件 + 区块 NBT）读写工具。
//!
//! 分层：
//! - [`region`] — .mca 容器：扇区表、区块定位、读写与压缩（zlib/gzip/未压缩）
//! - [`chunk`] — 区块 NBT：section 定位、方块/生物群系调色板读写（基于名称引用）
//! - [`palette`] — 通用调色板容器位打包编解码
//! - [`paths`] — 维度目录与区域文件名约定
//!
//! 方块状态以 [`BlockStateRef`]（名称 + 属性）表达，不依赖任何服务端注册表，
//! 因此可以读写任意版本/任意模组的存档。配合 [`qexed_nbt`] 的 `from_slice`/`to_vec`
//! 完成区块 NBT 与字节的互转。
//!
//! # 示例：读取一个方块
//!
//! ```no_run
//! use qexed_anvil::region::AnvilRegion;
//! use qexed_anvil::chunk;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let region = AnvilRegion::from_file("world/region/r.0.0.mca")?;
//! if let Some(data) = region.read_chunk(0, 0)? {
//!     let raw = data.decompress()?;
//!     let (_, root) = qexed_nbt::from_slice(&raw)?;
//!     let state = chunk::block_state_at(&root, 5, 70, 9)?; // 世界坐标
//!     println!("{state:?}");
//! }
//! # Ok(())
//! # }
//! ```

pub mod chunk;
pub mod error;
pub mod migrate;
pub mod palette;
pub mod paths;
pub mod region;
pub mod world;
pub mod upgrade;

#[cfg(feature = "qexed")]
pub mod config;

pub use chunk::{BlockStateRef, SectionBlocks, SectionBiomes};
pub use error::AnvilError;
pub use region::{AnvilRegion, ChunkData};
