// 存储修饰生物属性的属性修饰符，当物品在生物的指定槽位上时可以修改其所在生物的属性。物品存储的属性修饰符信息会在物品提示框中显示。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct AttributeModifiers {
    pub amount:f64,
    pub display:Option<Display>,
    pub id:String,
    pub operation:String,
    #[serde(default="default_slot")]
    pub slot:String,
    #[serde(rename="type")]
    pub r#type:String
}
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct Display {
    #[serde(rename="type")]
    pub r#type:String,

    pub value:String,
}
fn default_slot() -> String {
    "any".to_string()
}