use serde::{Deserialize, Serialize};

use crate::{
    public::{
        mongodb::MongoConfig, mysql::MysqlConfig, pika::PikaConfig, storage_engine::StorageEngine,
    },
    tool::AppConfigTrait,
};
#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    pub version: i32,
    pub storage_engine: StorageEngine,
    pub simple: Simple,
    pub mysql: Mysql,
    pub mongodb: MongoDB,
    pub pika: Pika,
}
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Simple {
    pub player_list:Vec<uuid::Uuid>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Mysql {
    #[serde(flatten)]
    pub data: MysqlConfig,
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

#[derive(Debug,Default, Serialize, Deserialize)]
pub struct MongoDB {
    #[serde(flatten)]
    pub data: MongoConfig,
}
#[derive(Debug, Serialize, Deserialize)]
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
impl AppConfigTrait for Data {
    const PATH: &'static str = "./config/qexed_wardon/database/";

    const NAME: &'static str = "config";
}
