use serde::{Deserialize, Serialize};

use crate::{
    public::{
        mongodb::MongoConfig, mysql::MysqlConfig, pika::PikaConfig, storage_engine::StorageEngine,
    },
    tool::AppConfigTrait,
};
#[derive(Debug, Serialize, Deserialize)]
pub struct BlackList {
    pub version: i32,
    pub storage_engine: StorageEngine,
    pub kick_message: String,
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
            table_prefix: "black_list".to_string(),
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
            key_prefix: "black_list".to_string(),
        }
    }
}

impl Default for BlackList {
    fn default() -> Self {
        Self {
            version: 0,
            kick_message: "您已被服务器拉入黑名单,禁止进入！！！"
            .to_string(),
            storage_engine: StorageEngine::Simple,
            simple: Default::default(),
            mysql: Default::default(),
            mongodb: Default::default(),
            pika: Default::default(),
        }
    }
}
impl AppConfigTrait for BlackList {
    const PATH: &'static str = "./config/qexed_whilelist/";

    const NAME: &'static str = "config";
}
