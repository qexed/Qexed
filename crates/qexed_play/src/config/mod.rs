use qexed_doc_macros::{Doc, DocValue};
use serde::{Deserialize, Serialize};

/// 出生区块地形来源：决定玩家出生/跑图拿到的区块内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
#[serde(rename_all = "snake_case")]
pub enum ChunkSource {
    /// 服务端内存构造的空区块（默认：超平占位，不依赖任何外部服务）。
    Empty,
    /// 接入 chunk-summon 服务（tools/mc-vanilla-chunk-summon）：qexed 作为
    /// TCP 客户端按需请求——服务端存档命中读盘、未命中原版生成，玩家
    /// 跑图任意坐标即取即得。
    ChunkTcp,
}

impl Default for ChunkSource {
    fn default() -> Self {
        Self::Empty
    }
}

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_play -> qexed.crates.play.config.PlayConfig
/// ```autodoc
/// <Name>qexed.crates.play.config.PlayConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "play")]
#[derive(Debug, Clone, Serialize, Deserialize, Doc)]
pub struct PlayConfig {
    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.view_distance</Name>
    /// <Attr name="writable" />
    /// ```
    pub view_distance: i32,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.simulation_distance</Name>
    /// <Attr name="writable" />
    /// ```
    pub simulation_distance: i32,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.dimension_name</Name>
    /// <Attr name="writable" />
    /// ```
    pub dimension_name: String,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.dimension_type</Name>
    /// <Attr name="writable" />
    /// ```
    pub dimension_type: i32,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.hashed_seed</Name>
    /// <Attr name="writable" />
    /// ```
    pub hashed_seed: i64,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.game_mode</Name>
    /// <Attr name="writable" />
    /// ```
    pub game_mode: u8,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.chunk_source</Name>
    /// <Attr name="writable" />
    /// ```
    #[serde(default)]
    pub chunk_source: ChunkSource,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.chunk_tcp_addr</Name>
    /// <Attr name="writable" />
    /// ```
    #[serde(default = "default_chunk_tcp_addr")]
    pub chunk_tcp_addr: String,

    /// ```autodoc
    /// <Name>qexed.crates.play.config.PlayConfig.chunk_tcp_fallback_empty</Name>
    /// <Attr name="writable" />
    /// ```
    #[serde(default = "default_true")]
    pub chunk_tcp_fallback_empty: bool,
}

fn default_chunk_tcp_addr() -> String {
    "127.0.0.1:54132".to_string()
}

fn default_true() -> bool {
    true
}
impl Default for PlayConfig {
    fn default() -> Self {
        Self {
            view_distance: 8,
            simulation_distance: 8,
            dimension_name: "minecraft:overworld".to_string(),
            dimension_type: 0,
            hashed_seed: 0,
            game_mode: 1,
            chunk_source: ChunkSource::Empty,
            chunk_tcp_addr: default_chunk_tcp_addr(),
            chunk_tcp_fallback_empty: true,
        }
    }
}
