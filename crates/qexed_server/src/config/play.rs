use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

/// PLAY 阶段的服务器选项。
/// ```autodoc
/// <Name>qexed.crates.server.config.PlayOptionsConfig</Name>
/// ```
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct PlayOptionsConfig {
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayOptionsConfig.view_distance</Name>
    /// <Default>8</Default>
    /// <Attr name="writable" />
    /// ```
    pub view_distance: i32,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayOptionsConfig.simulation_distance</Name>
    /// <Default>8</Default>
    /// <Attr name="writable" />
    /// ```
    pub simulation_distance: i32,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayOptionsConfig.dimension_name</Name>
    /// <Default>"minecraft:overworld"</Default>
    /// <Attr name="writable" />
    /// ```
    pub dimension_name: String,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayOptionsConfig.dimension_type</Name>
    /// <Default>0</Default>
    /// <Attr name="writable" />
    /// ```
    pub dimension_type: i32,
    /// ```autodoc
    /// <Name>qexed.crates.server.config.PlayOptionsConfig.hashed_seed</Name>
    /// <Default>0</Default>
    /// <Attr name="writable" />
    /// ```
    pub hashed_seed: i64,
}

impl Default for PlayOptionsConfig {
    fn default() -> Self {
        Self {
            view_distance: 8,
            simulation_distance: 8,
            dimension_name: "minecraft:overworld".to_string(),
            dimension_type: 0,
            hashed_seed: 0,
        }
    }
}
