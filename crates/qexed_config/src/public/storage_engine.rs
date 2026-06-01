use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum StorageEngine {
    Simple,
    Mysql,
    MongoDB,
    Pika,
}

impl Default for StorageEngine {
    fn default() -> Self {
        StorageEngine::Simple
    }
}

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

impl std::str::FromStr for StorageEngine {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(StorageEngine::Simple),
            "mysql" => Ok(StorageEngine::Mysql),
            "mongodb" => Ok(StorageEngine::MongoDB),
            "pika" => Ok(StorageEngine::Pika),
            _ => Err(format!("未知引擎? {}", s)),
        }
    }
}
