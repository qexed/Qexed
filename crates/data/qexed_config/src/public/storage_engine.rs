use serde::Deserialize;
use serde::Serialize;
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum StorageEngine {
    Simple,// 简易,项目内置
    Mysql,// Mysql数据库
    MongoDB,// MongoDB数据库
    Pika, // Pika数据库(Redis协议)
}
impl Default for StorageEngine {
    fn default() -> Self {
        StorageEngine::Simple
    }
}

// 为ForwardingMode实现Display trait
impl std::fmt::Display for StorageEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageEngine::Simple => write!(f, "Simple"),
            StorageEngine::Mysql => write!(f, "Mysql"),
            StorageEngine::MongoDB => write!(f, "MongoDB"),
            StorageEngine::Pika => write!(f, "Pika"),
        }
    }
}

// 为ForwardingMode实现FromStr用于解析
impl std::str::FromStr for StorageEngine {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(StorageEngine::Simple),
            "mysql" => Ok(StorageEngine::Mysql),
            "mongodb" => Ok(StorageEngine::MongoDB),
            "pika" => Ok(StorageEngine::Pika),
            _ => Err(format!("未知的转发模式: {}", s)),
        }
    }
}