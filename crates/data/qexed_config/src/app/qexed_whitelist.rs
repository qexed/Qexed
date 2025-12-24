use serde::{Deserialize, Serialize};

use crate::{
    public::{
        mongodb::MongoConfig, mysql::MysqlConfig, pika::PikaConfig, storage_engine::StorageEngine,
    },
    tool::AppConfigTrait,
};
#[derive(Debug, Serialize, Deserialize)]
pub struct WhiteList {
    pub version: i32,
    pub enable: bool,
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
            table_prefix: "white_list".to_string(),
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
            key_prefix: "white_list".to_string(),
        }
    }
}

impl Default for WhiteList {
    fn default() -> Self {
        Self {
            version: 0,
            enable: false,
            kick_message: "
§c==================================
§6§l服务器白名单系统
§c==================================
§f抱歉，§e{player}§f，你不在服务器的白名单中
§fIP地址: §7{ip}
§f服务器: §e我的世界服务器
§f时间: §7{time}
§c==================================
§7请先联系管理员申请加入白名单
§c==================================
"
            .to_string(),
            storage_engine: StorageEngine::Simple,
            simple: Default::default(),
            mysql: Default::default(),
            mongodb: Default::default(),
            pika: Default::default(),
        }
    }
}
impl AppConfigTrait for WhiteList {
    const PATH: &'static str = "./config/qexed_whilelist/";

    const NAME: &'static str = "config";
}
