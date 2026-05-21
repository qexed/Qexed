use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct IP {
    #[cfg(feature = "distributed")]
    #[AutoDoc(key = "config.public.ip.ip")]
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
