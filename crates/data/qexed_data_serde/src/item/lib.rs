// 使用正确的导入路径
pub mod components;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct Slot {
    pub id:String,
    pub components:Option<components::Components>,
    pub count:Option<i32>,
}

// /give @s golden_axe[tooltip_display={hide_tooltip:false,hidden_components:[enchantments]}]