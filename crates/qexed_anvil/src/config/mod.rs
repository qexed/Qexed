//! Qexed 内部配置集成（`qexed` 特征）。

use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
/// ```autodoc
/// <Name>qexed.crates.anvil.config.AnvilConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "anvil")]
#[derive(Debug, Default, Serialize, Deserialize, Doc)]
pub struct AnvilConfig {}
