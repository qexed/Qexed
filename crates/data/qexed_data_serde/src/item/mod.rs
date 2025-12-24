mod components;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    /// （命名空间ID）类型。
    pub id: String,
    pub components:Box<components::Components>,
    #[serde(default="count")]
    pub count:i32,
    pub slot:Option<u8>
}

fn count()->i32{1}