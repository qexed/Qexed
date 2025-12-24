use serde::Serialize;
use serde::Deserialize;
use std::collections::HashMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStates{
    #[serde(rename="Name")]
    pub name:String,
    #[serde(rename="Properties")]
    pub properties:Option<HashMap<String,String>>,
}