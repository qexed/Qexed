pub mod minecraft;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
    #[serde(rename="minecraft:attribute_modifiers")]
    pub attributemodifiers: Option<Vec<minecraft::attribute_modifiers::AttributeModifiers>>,
    #[serde(rename="minecraft:banner_patterns")]
    pub banner_patterns:Option<Vec<minecraft::banner_patterns::BannerPatternLayer>>,
    #[serde(rename="minecraft:base_color")]
    pub base_color:Option<String>,
    #[serde(rename="minecraft:bees")]
    pub bee:Option<minecraft::bees::Bees>,
    #[serde(rename="minecraft:block_entity_data")]
    pub block_entity_data:Option<minecraft::block_entity_data::BlockEntityData>
    // 后续完善，目前阶段不着急
}