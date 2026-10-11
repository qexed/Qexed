use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// 天光算法（仅 Dynamic 模式生效）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum LightAlgorithm {
    /// 列扫描 + 邻域播种 + BFS（快）。
    Fast,
    /// Fast + 斜向射线追踪（慢但洞口/悬崖更真实）。
    RayTrace,
}

impl Default for LightAlgorithm {
    fn default() -> Self {
        Self::Fast
    }
}

impl From<LightAlgorithm> for crate::engine::SkyLightAlgorithm {
    fn from(value: LightAlgorithm) -> Self {
        match value {
            LightAlgorithm::Fast => Self::Fast,
            LightAlgorithm::RayTrace => Self::RayTrace,
        }
    }
}
