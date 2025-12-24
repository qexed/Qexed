// 存储旗帜和盾牌上的旗帜图案。旗帜图案信息会在提示框中显示。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)] // 表示不包含类型标签，根据数据本身判断是哪种变体
pub enum BannerPatternVariant {
    /// 官方图案：直接使用图案ID字符串，例如 "flower", "creeper"
    Official(String),
    /// 自定义图案：使用内联对象，包含 asset_id 和 translation_key
    Custom(Box<CustomPattern>),
}

/// 自定义图案的内联对象格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPattern {
    /// 图案渲染时所使用的纹理的命名空间ID[citation:2]
    pub asset_id: String,
    /// 物品提示框内文本的翻译键前缀[citation:2]
    pub translation_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannerPatternLayer {
    /// 这一层图案的颜色，取值为染料颜色，例如 "red", "blue"
    pub color: String,
    /// 这一层图案的样式
    pub pattern: BannerPatternVariant,
}