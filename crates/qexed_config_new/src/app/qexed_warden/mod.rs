pub mod data;

use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

#[qexed_config_macros::app_config("/", "qexed_warden")]
#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct QexedWarden {
    #[AutoDoc(key = "config.qexed_warden.version")]
    pub version: i32,

    #[AutoDoc(key = "config.qexed_warden.data", sub)]
    pub data: data::Data,
}

impl Default for QexedWarden {
    fn default() -> Self {
        Self {
            version: Default::default(),
            data: Default::default(),
        }
    }
}
