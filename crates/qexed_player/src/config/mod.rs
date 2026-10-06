use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// 玩家数据存储引擎。
/// v4 对应 qexed_config::app::qexed::server::PlayerDataEngine。
/// TODO(storage): Mongodb/Mysql 变体在 player_data 后端依赖落地前只做配置占位
/// （运行时回退 DisabledPlayerDataStore，见 player_data/mod.rs）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
#[serde(rename_all = "snake_case")]
pub enum PlayerDataEngine {
    Vanilla,
    Mongodb,
    Mysql,
}

impl Default for PlayerDataEngine {
    fn default() -> Self {
        Self::Vanilla
    }
}

/// 玩家数据持久化配置（v4 对应 qexed_config::app::qexed::server::PlayerData）。
/// v4 的 mongodb/mysql 连接配置（MongoConfig/MysqlConfig）在 v6 的 qexed_config
/// 中不存在，等 storage 依赖引入后再补；此处仅保留引擎选择与表名/集合名。
///
/// ```autodoc
/// <Name>qexed.crates.player.config.PlayerDataConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "player_data")]
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerDataConfig {
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.enable</Name>
    /// <Default>true</Default>
    /// ```
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.autosave_interval_secs</Name>
    /// <Default>120</Default>
    /// ```
    pub autosave_interval_secs: u64,
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.engine</Name>
    /// <Default>Vanilla</Default>
    /// ```
    pub engine: PlayerDataEngine,
    /// mongodb 集合名（TODO(storage)：引擎落地前仅配置占位）。
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.collection</Name>
    /// <Default>qexed_players</Default>
    /// ```
    pub collection: String,
    /// mysql 表名（TODO(storage)：引擎落地前仅配置占位）。
    /// ```autodoc
    /// <Name>qexed.crates.player.config.PlayerDataConfig.table</Name>
    /// <Default>qexed_player_data</Default>
    /// ```
    pub table: String,
}

impl Default for PlayerDataConfig {
    fn default() -> Self {
        Self {
            enable: true,
            autosave_interval_secs: 120,
            engine: PlayerDataEngine::default(),
            collection: "qexed_players".to_string(),
            table: "qexed_player_data".to_string(),
        }
    }
}
