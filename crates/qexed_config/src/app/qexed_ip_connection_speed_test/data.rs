use serde::{Deserialize, Serialize};

use crate::{
    public::{
        pika::PikaConfig, storage_engine::StorageEngine,
    },
    tool::AppConfigTrait,
};
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct Data {
    pub version: i32,
    pub storage_engine: StorageEngine,
    // 有点难,这部分我不打算写死在配置文件中
    pub simple: Simple,
    pub pika: Pika,
}
#[derive(Default, Debug, Serialize, Deserialize,Clone)]
pub struct Simple {
    pub player_list:Vec<uuid::Uuid>,
}

#[derive(Debug, Serialize, Deserialize,Clone)]
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
impl AppConfigTrait for Data {
    const PATH: &'static str = "./config/qexed_wardon/database/";

    const NAME: &'static str = "config";
}
