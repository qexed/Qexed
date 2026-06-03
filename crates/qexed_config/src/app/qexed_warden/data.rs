use serde::{Deserialize, Serialize};

use crate::public::{
    mongodb::MongoConfig, mysql::MysqlConfig, pika::PikaConfig, storage_engine::StorageEngine,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub version: i32,

    #[serde(default)]
    pub storage_engine: StorageEngine,

    #[serde(default)]
    pub simple: Simple,

    #[serde(default)]
    pub mysql: Mysql,

    #[serde(default)]
    pub mongodb: MongoDB,

    #[serde(default)]
    pub pika: Pika,
}

impl crate::tool::AppConfigTrait for Data {
    const PATH: &'static str = "/qexed_wardon/database/";
    const NAME: &'static str = "config";
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Simple {
    #[serde(default)]
    pub player_list: Vec<uuid::Uuid>,
    #[serde(default)]
    pub bans: Vec<BanRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BanRecord {
    pub uuid: uuid::Uuid,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub permanent: bool,
    #[serde(default)]
    pub created_at_unix_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mysql {
    #[serde(flatten)]
    pub data: MysqlConfig,
    #[serde(default = "default_table_prefix")]
    pub table_prefix: String,
}

fn default_table_prefix() -> String {
    "wardon".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MongoDB {
    #[serde(flatten)]
    pub data: MongoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for Mysql {
    fn default() -> Self {
        Self {
            data: Default::default(),
            table_prefix: default_table_prefix(),
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
