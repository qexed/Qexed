use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// 光照模式（对接 qexed_world 发 chunk 时的光照来源）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum LightMode {
    /// 静态：全 chunk 天光 15 / 方块光 0（v4 Static）。
    Static,
    /// 动态：按方块数据实际计算（v4 Dynamic，即本 crate 引擎）。
    Dynamic,
    /// 固定等级：天光/方块光都设为 fixed_level（v4 Fixed）。
    Fixed,
}

impl Default for LightMode {
    fn default() -> Self {
        Self::Static
    }
}
