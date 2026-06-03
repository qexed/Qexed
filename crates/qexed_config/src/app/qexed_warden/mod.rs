pub mod data;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedWarden {
    #[serde(default)]
    pub version: i32,

    #[serde(default)]
    pub data: data::Data,
}

impl crate::tool::AppConfigTrait for QexedWarden {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_warden";
}

impl Default for QexedWarden {
    fn default() -> Self {
        Self {
            version: Default::default(),
            data: Default::default(),
        }
    }
}
