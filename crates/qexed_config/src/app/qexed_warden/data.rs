use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::public::{
    mongodb::MongoConfig, mysql::MysqlConfig, pika::PikaConfig, storage_engine::StorageEngine,
};

#[qexed_config_macros::app_config("/qexed_wardon/database/", "config")]
#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct Data {
    #[AutoDoc(key = "config.qexed_warden.data.version")]
    pub version: i32,

    #[AutoDoc(key = "config.qexed_warden.data.storage_engine")]
    pub storage_engine: StorageEngine,

    #[AutoDoc(key = "config.qexed_warden.data.simple", sub)]
    pub simple: Simple,

    #[AutoDoc(key = "config.qexed_warden.data.mysql", sub)]
    pub mysql: Mysql,

    #[AutoDoc(key = "config.qexed_warden.data.mongodb", sub)]
    pub mongodb: MongoDB,

    #[AutoDoc(key = "config.qexed_warden.data.pika", sub)]
    pub pika: Pika,
}

#[derive(Default, Debug, Serialize, Deserialize, AutoDoc)]
pub struct Simple {
    #[AutoDoc(key = "config.qexed_warden.data.simple.player_list")]
    pub player_list: Vec<uuid::Uuid>,
}

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct Mysql {
    #[serde(flatten)]
    pub data: MysqlConfig,
    #[AutoDoc(key = "config.qexed_warden.data.mysql.table_prefix")]
    pub table_prefix: String,
}

impl Default for Mysql {
    fn default() -> Self {
        Self {
            data: Default::default(),
            table_prefix: "wardon".to_string(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, AutoDoc)]
pub struct MongoDB {
    #[serde(flatten)]
    pub data: MongoConfig,
}

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct Pika {
    #[serde(flatten)]
    pub data: PikaConfig,
    #[serde(default)]
    #[AutoDoc(key = "config.qexed_warden.data.pika.key_prefix")]
    pub key_prefix: String,
}

impl Default for Pika {
    fn default() -> Self {
        Self {
            data: Default::default(),
            key_prefix: "wardon".to_string(),
        }
    }
}

impl Default for Data {
    fn default() -> Self {
        Self {
            version: 0,
            storage_engine: StorageEngine::Simple,
            simple: Default::default(),
            mysql: Default::default(),
            mongodb: Default::default(),
            pika: Default::default(),
        }
    }
}
