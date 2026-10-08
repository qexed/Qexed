use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_mojang_data -> qexed.crates.mojang_data.config.MojangDataConfig
//   qexed_a           -> qexed.crates.a.config.AConfig
//   qexed_log         -> qexed.crates.log.config.LogConfig
/// ```autodoc
/// <Name>qexed.crates.protocol.config.ProtocolConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "protocol")]
#[derive(Debug, Default, Serialize, Deserialize, Doc)]
pub struct ProtocolConfig {}
