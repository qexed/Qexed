//! qexed_light：Minecraft 26.1.2 光照计算引擎（对接 qexed_world）。
//!
//! 职责边界（对齐 MIGRATION-PLAN 的依赖方向 world → light）：
//! - 输入：qexed_world 提供的方块光学属性（不透明度/亮度）3x3 chunk 邻域；
//! - 输出：整 chunk 天光/方块光 + 网络形态（section 掩码 + 2048 字节数组），
//!   协议无关纯数据，qexed_world 负责转 Bitset/LightArray 并打包
//!   MapChunk.Light / LightUpdate；
//! - 不依赖 qexed_protocol / qexed_packet（避免 world→light 之外的边）。
//!
//! 26.1.2 语义要点：
//! - 光照 section 网格 = 方块区间 ± 1 层（主世界 26 层：-5..=20）；
//! - 天光竖直无衰减、进入遮挡后按不透明度衰减、水平/向上 -1；
//! - has_skylight=false（下界）天光恒 0；
//! - 末地天光按高度满级（无遮挡时整列 15）。
//!
//! # 示例
//!
//! ```
//! use qexed_light::config::algorithm::LightAlgorithm;
//! use qexed_light::data::{BlockLightProps, ChunkOpacity, OpacityNeighborhood};
//! use qexed_light::engine::LightEngine;
//! use qexed_light::geometry::DimensionGeometry;
//!
//! let dim = DimensionGeometry::overworld();
//! let mut opacity = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
//! // 在 (8, 100, 8) 放一个实心方块
//! opacity.set(8, 100, 8, BlockLightProps::SOLID).unwrap();
//! let neighborhood = OpacityNeighborhood::single(opacity);
//! let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
//! let light = engine.compute_sky_light(&neighborhood);
//! // 方块自身全黑，正下方靠侧面绕光 15-1=14，其余直射 = 15
//! assert_eq!(light.sky_at(8, 100, 8), 0);
//! assert_eq!(light.sky_at(8, 99, 8), 14);
//! assert_eq!(light.sky_at(0, 0, 0), 15);
//! ```

pub mod config;
pub mod data;
pub mod engine;
pub mod error;
pub mod geometry;
pub mod output;

pub use config::algorithm::LightAlgorithm;
pub use config::mode::LightMode;
pub use data::{BlockLightProps, ChunkLightData, ChunkOpacity, LightNeighborhood, OpacityNeighborhood};
pub use engine::{LightEngine, SkyLightAlgorithm};
pub use error::LightError;
pub use geometry::{DimensionGeometry, LightGeometry};
pub use output::{LightPayload, SectionLightArray};
