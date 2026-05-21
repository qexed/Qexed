use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::public::{pika::PikaConfig, storage_engine::StorageEngine};

#[qexed_config_macros::app_config("/qexed_wardon/database/", "config")]
#[derive(Debug, Serialize, Deserialize, Clone, AutoDoc)]
pub struct Data {
    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data.version")]
    pub version: i32,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data.storage_engine")]
    pub storage_engine: StorageEngine,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data.simple", sub)]
    pub simple: Simple,

    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data.pika", sub)]
    pub pika: Pika,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, AutoDoc)]
pub struct Simple {
    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data.simple.player_list")]
    pub player_list: Vec<uuid::Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone, AutoDoc)]
pub struct Pika {
    #[serde(flatten)]
    pub data: PikaConfig,
    #[serde(default)]
    #[AutoDoc(key = "config.qexed_ip_connection_speed_test.data.pika.key_prefix")]
    pub key_prefix: String,
}

impl Default for Pika {
    fn default() -> Self {
        Self {
            data: Default::default(),
            key_prefix: "ip_connection_speed_test".to_string(),
        }
    }
}

impl Default for Data {
    fn default() -> Self {
        Self {
            version: 0,
            storage_engine: StorageEngine::Simple,
            simple: Default::default(),
            pika: Default::default(),
        }
    }
}
