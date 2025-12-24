use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;

#[derive(Debug, Serialize, Deserialize)]
pub struct IP {
    #[cfg(feature = "distributed")]
    pub ip: String,
}
impl Default for IP {
    fn default() -> Self {
        Self {
            #[cfg(feature = "distributed")]
            ip: "0.0.0.0:25565".to_string(),
        }
    }
}
