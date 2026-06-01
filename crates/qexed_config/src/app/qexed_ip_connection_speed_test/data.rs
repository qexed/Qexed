use serde::{Deserialize, Serialize};

use crate::public::{pika::PikaConfig, storage_engine::StorageEngine};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Data {
    pub version: i32,

    pub storage_engine: StorageEngine,

    pub simple: Simple,

    pub pika: Pika,
}

impl crate::tool::AppConfigTrait for Data {
    const PATH: &'static str = "/qexed_wardon/database/";
    const NAME: &'static str = "config";
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Simple {
    pub player_list: Vec<uuid::Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pika {
    #[serde(flatten)]
    pub data: PikaConfig,
    #[serde(default)]
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
