pub mod data;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct QexedWarden {
    pub version: i32,

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
