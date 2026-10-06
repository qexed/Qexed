//! 插件结构化存储（v4 plugins/structured_storage.rs 迁移）。
//!
//! 后端：sqlite 枚举位由文件后端承载（v6 无 rusqlite），mysql / mongodb 为
//! v4 实现的完整迁移。异步驱动在同步 trait 后面各挂一个专用的 current_thread
//! tokio Runtime（与 v4 相同的结构），连接失败回退文件后端并 warn。

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::config::StructuredStorageConfig;

#[derive(Debug)]
pub(crate) struct StructuredStorageState {
    backend: Arc<dyn StructuredStorageBackend>,
}

impl Default for StructuredStorageState {
    fn default() -> Self {
        Self {
            backend: Arc::new(FileStructuredStorage::new_default()),
        }
    }
}

impl StructuredStorageState {
    pub(crate) fn configure(&mut self, config: &StructuredStorageConfig) {
        match build_backend(config) {
            Ok(backend) => self.backend = backend,
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.storage.unavailable")
                        .replace("%{error}", &err.to_string())
                );
                self.backend = Arc::new(FileStructuredStorage::new_default());
            }
        }
    }

    pub(crate) fn exists(&self, plugin: &str, key: &str) -> bool {
        self.backend.exists(plugin, key).unwrap_or(false)
    }

    pub(crate) fn get(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        self.backend.get(plugin, key).ok().flatten()
    }

    pub(crate) fn set(&self, plugin: &str, key: &str, value: &[u8]) -> bool {
        self.backend.set(plugin, key, value).is_ok()
    }

    pub(crate) fn delete(&self, plugin: &str, key: &str) -> bool {
        self.backend.delete(plugin, key).is_ok()
    }
}

fn build_backend(
    config: &StructuredStorageConfig,
) -> Result<Arc<dyn StructuredStorageBackend>, crate::error::PluginsError> {
    match config.engine {
        crate::config::StructuredStorageEngine::Sqlite => {
            Ok(Arc::new(FileStructuredStorage::new_default()))
        }
        crate::config::StructuredStorageEngine::Mysql => {
            Ok(Arc::new(MysqlStructuredStorage::new(&config.mysql)?))
        }
        crate::config::StructuredStorageEngine::Mongodb => {
            Ok(Arc::new(MongoStructuredStorage::new(&config.mongodb)?))
        }
    }
}

/// 结构化存储接口（文件 / mysql / mongodb 三后端实现）。
pub(crate) trait StructuredStorageBackend: Send + Sync + std::fmt::Debug {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool, crate::error::PluginsError>;
    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>, crate::error::PluginsError>;
    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<(), crate::error::PluginsError>;
    fn delete(&self, plugin: &str, key: &str) -> Result<(), crate::error::PluginsError>;
}

/// 文件后端：config/plugins/<plugin>/structured/<key> 一键一文件。
#[derive(Debug)]
struct FileStructuredStorage {
    root: PathBuf,
}

impl FileStructuredStorage {
    fn new_default() -> Self {
        Self {
            root: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("config")
                .join("plugins"),
        }
    }

    fn path_for(&self, plugin: &str, key: &str) -> Result<PathBuf, crate::error::PluginsError> {
        let clean = clean_storage_key(key).ok_or_else(|| {
            crate::error::PluginsError::StructuredStorage(format!("invalid key: {key}"))
        })?;
        Ok(self.root.join(plugin).join("structured").join(clean))
    }
}

impl StructuredStorageBackend for FileStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool, crate::error::PluginsError> {
        Ok(self.path_for(plugin, key)?.is_file())
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>, crate::error::PluginsError> {
        match fs::read(self.path_for(plugin, key)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<(), crate::error::PluginsError> {
        let path = self.path_for(plugin, key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, value)?;
        Ok(())
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<(), crate::error::PluginsError> {
        match fs::remove_file(self.path_for(plugin, key)?) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

/// MySQL 后端（v4 MysqlStructuredStorage 迁移；mysql_async 0.36 无 params! 宏，
/// 统一改用 positional 元组参数）。
#[derive(Debug)]
struct MysqlStructuredStorage {
    runtime: Mutex<tokio::runtime::Runtime>,
    pool: mysql_async::Pool,
}

impl MysqlStructuredStorage {
    fn new(config: &crate::config::MysqlStorageConfig) -> Result<Self, crate::error::PluginsError> {
        config
            .validate()
            .map_err(crate::error::PluginsError::StructuredStorage)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                crate::error::PluginsError::StructuredStorage(format!("build mysql runtime: {err}"))
            })?;
        let opts = mysql_async::Opts::from(
            mysql_async::OptsBuilder::default()
                .ip_or_hostname(config.ip.clone())
                .tcp_port(config.port)
                .user(Some(config.username.clone()))
                .pass(Some(config.password.clone()))
                .db_name(Some(config.database.clone())),
        );
        let pool = runtime
            .block_on(async {
                let pool = mysql_async::Pool::new(opts);
                ensure_mysql_table(&pool).await?;
                Ok::<_, crate::error::PluginsError>(pool)
            })
            .map_err(|err| {
                crate::error::PluginsError::StructuredStorage(format!("mysql connect: {err}"))
            })?;
        Ok(Self {
            runtime: Mutex::new(runtime),
            pool,
        })
    }

    fn block_on<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, crate::error::PluginsError>>,
    ) -> Result<T, crate::error::PluginsError> {
        self.runtime
            .lock()
            .expect("mysql plugin storage runtime poisoned")
            .block_on(future)
    }
}

async fn ensure_mysql_table(pool: &mysql_async::Pool) -> Result<(), crate::error::PluginsError> {
    use mysql_async::prelude::Queryable;

    let mut conn = pool
        .get_conn()
        .await
        .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `qexed_plugin_structured_storage` (
            `plugin` VARCHAR(128) NOT NULL,
            `storage_key` VARCHAR(512) NOT NULL,
            `value` LONGBLOB NOT NULL,
            `updated_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (`plugin`, `storage_key`)
        )",
    )
    .await
    .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
    Ok(())
}

impl StructuredStorageBackend for MysqlStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool, crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let plugin = plugin.to_string();
        let key = key.to_string();
        self.block_on(async {
            let mut conn = self
                .pool
                .get_conn()
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            let count: Option<i64> = conn
                .exec_first(
                    "SELECT COUNT(*) FROM `qexed_plugin_structured_storage`
                     WHERE `plugin` = ? AND `storage_key` = ?",
                    (&plugin, &key),
                )
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(count.unwrap_or(0) > 0)
        })
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>, crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let plugin = plugin.to_string();
        let key = key.to_string();
        self.block_on(async {
            let mut conn = self
                .pool
                .get_conn()
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(conn
                .exec_first(
                    "SELECT `value` FROM `qexed_plugin_structured_storage`
                     WHERE `plugin` = ? AND `storage_key` = ?",
                    (&plugin, &key),
                )
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?)
        })
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<(), crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let plugin = plugin.to_string();
        let key = key.to_string();
        let value = value.to_vec();
        self.block_on(async {
            let mut conn = self
                .pool
                .get_conn()
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            conn.exec_drop(
                "INSERT INTO `qexed_plugin_structured_storage` (`plugin`, `storage_key`, `value`)
                 VALUES (?, ?, ?)
                 ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)",
                (&plugin, &key, &value),
            )
            .await
            .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(())
        })
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<(), crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let plugin = plugin.to_string();
        let key = key.to_string();
        self.block_on(async {
            let mut conn = self
                .pool
                .get_conn()
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            conn.exec_drop(
                "DELETE FROM `qexed_plugin_structured_storage`
                 WHERE `plugin` = ? AND `storage_key` = ?",
                (&plugin, &key),
            )
            .await
            .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(())
        })
    }
}

/// MongoDB 后端（v4 MongoStructuredStorage 迁移）。
#[derive(Debug)]
struct MongoStructuredStorage {
    runtime: Mutex<tokio::runtime::Runtime>,
    client: mongodb::Client,
    database: String,
}

impl MongoStructuredStorage {
    fn new(config: &crate::config::MongoStorageConfig) -> Result<Self, crate::error::PluginsError> {
        config
            .validate()
            .map_err(crate::error::PluginsError::StructuredStorage)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                crate::error::PluginsError::StructuredStorage(format!("build mongo runtime: {err}"))
            })?;
        let client = runtime
            .block_on(async { mongo_client(config).await })
            .map_err(|err| {
                crate::error::PluginsError::StructuredStorage(format!("mongo connect: {err}"))
            })?;
        // mongodb 驱动懒连接：new 里主动 ping 一次（限时），让不可达在 configure
        // 阶段就暴露，触发文件后端回退，而不是首次读写才失败。
        runtime
            .block_on(async {
                let ping = tokio::time::timeout(
                    std::time::Duration::from_millis(config.connect_timeout_ms),
                    client
                        .database(&config.database)
                        .run_command(mongodb::bson::doc! { "ping": 1 }),
                )
                .await
                .map_err(|_| crate::error::PluginsError::StructuredStorage("mongo ping timed out".to_string()))?
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
                Ok::<_, crate::error::PluginsError>(ping)
            })
            .map_err(|err| {
                crate::error::PluginsError::StructuredStorage(format!("mongo ping: {err}"))
            })?;
        Ok(Self {
            runtime: Mutex::new(runtime),
            client,
            database: config.database.clone(),
        })
    }

    fn collection(&self) -> mongodb::Collection<mongodb::bson::Document> {
        self.client
            .database(&self.database)
            .collection("qexed_plugin_structured_storage")
    }

    fn block_on<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, crate::error::PluginsError>>,
    ) -> Result<T, crate::error::PluginsError> {
        self.runtime
            .lock()
            .expect("mongo plugin storage runtime poisoned")
            .block_on(future)
    }
}

impl StructuredStorageBackend for MongoStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool, crate::error::PluginsError> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        self.block_on(async {
            let count = collection
                .count_documents(mongodb::bson::doc! { "_id": id })
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(count > 0)
        })
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>, crate::error::PluginsError> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        self.block_on(async {
            let Some(doc) = collection
                .find_one(mongodb::bson::doc! { "_id": id })
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?
            else {
                return Ok(None);
            };
            let value = doc
                .get_binary_generic("value")
                .map_err(|err| {
                    crate::error::PluginsError::StructuredStorage(format!(
                        "Mongo plugin storage document missing binary value: {err}",
                    ))
                })?
                .to_vec();
            Ok(Some(value))
        })
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<(), crate::error::PluginsError> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        let plugin = plugin.to_string();
        let key = key.to_string();
        let value = value.to_vec();
        self.block_on(async {
            collection
                .update_one(
                    mongodb::bson::doc! { "_id": id },
                    mongodb::bson::doc! {
                        "$set": {
                            "plugin": plugin,
                            "key": key,
                            "value": mongodb::bson::Binary {
                                subtype: mongodb::bson::spec::BinarySubtype::Generic,
                                bytes: value,
                            },
                            "updated_at": mongodb::bson::DateTime::now(),
                        }
                    },
                )
                .upsert(true)
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(())
        })
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<(), crate::error::PluginsError> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        self.block_on(async {
            collection
                .delete_one(mongodb::bson::doc! { "_id": id })
                .await
                .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
            Ok(())
        })
    }
}

async fn mongo_client(
    config: &crate::config::MongoStorageConfig,
) -> Result<mongodb::Client, crate::error::PluginsError> {
    let mut options = mongodb::options::ClientOptions::parse(config.connection_uri())
        .await
        .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))?;
    options.app_name = Some("qexed".to_string());
    options.connect_timeout = Some(std::time::Duration::from_millis(
        config.connect_timeout_ms,
    ));
    if let Some(auth_source) = &config.auth_source {
        if let Some(credential) = &mut options.credential {
            credential.source = Some(auth_source.clone());
        }
    }
    mongodb::Client::with_options(options)
        .map_err(|err| crate::error::PluginsError::StructuredStorage(err.to_string()))
}

fn storage_id(plugin: &str, key: &str) -> String {
    format!("{plugin}\t{key}")
}

fn clean_storage_key(key: &str) -> Option<PathBuf> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    let mut clean = PathBuf::new();
    for segment in key.split('/') {
        let segment = segment.trim();
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('\\')
            || segment.contains(':')
        {
            return None;
        }
        let safe = segment
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>();
        if safe.is_empty() {
            return None;
        }
        clean.push(safe);
    }
    clean.set_extension("bin");
    Some(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(backend: &dyn StructuredStorageBackend) {
        let plugin = "structured_storage_test_plugin";
        let key = format!(
            "roundtrip-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        );
        let value = b"\x00typed-json-like-value\xff";
        let updated = b"updated-value";

        assert!(!backend.exists(plugin, &key).expect("exists before set"));
        assert_eq!(backend.get(plugin, &key).expect("get before set"), None);

        backend.set(plugin, &key, value).expect("set value");
        assert!(backend.exists(plugin, &key).expect("exists after set"));
        assert_eq!(
            backend.get(plugin, &key).expect("get after set").as_deref(),
            Some(value.as_slice()),
        );

        backend.set(plugin, &key, updated).expect("update value");
        assert_eq!(
            backend
                .get(plugin, &key)
                .expect("get after update")
                .as_deref(),
            Some(updated.as_slice()),
        );

        backend.delete(plugin, &key).expect("delete value");
        assert!(!backend.exists(plugin, &key).expect("exists after delete"));
        assert_eq!(backend.get(plugin, &key).expect("get after delete"), None);
    }

    #[test]
    fn file_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "qexed-plugins-ss-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let backend = FileStructuredStorage { root: dir };
        roundtrip(&backend);
    }

    /// 连接失败回退文件后端并 warn（不 panic）：engine 指向不可达端口，
    /// configure 应静默落到 FileStructuredStorage。
    #[test]
    fn mysql_engine_falls_back_to_file_when_connect_fails() {
        let mut state = StructuredStorageState::default();
        let config = StructuredStorageConfig {
            engine: crate::config::StructuredStorageEngine::Mysql,
            mysql: crate::config::MysqlStorageConfig {
                port: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        state.configure(&config);

        // 回退后仍可读写（落文件后端）。
        assert!(state.set("plugin", "fallback-key", b"value"));
        assert_eq!(state.get("plugin", "fallback-key"), Some(b"value".to_vec()));
        assert!(state.delete("plugin", "fallback-key"));
    }

    #[test]
    fn mongodb_engine_falls_back_to_file_when_connect_fails() {
        let mut state = StructuredStorageState::default();
        let config = StructuredStorageConfig {
            engine: crate::config::StructuredStorageEngine::Mongodb,
            mongodb: crate::config::MongoStorageConfig {
                port: 1,
                connect_timeout_ms: 200,
                ..Default::default()
            },
            ..Default::default()
        };

        state.configure(&config);

        assert!(state.set("plugin", "fallback-key", b"value"));
        assert_eq!(state.get("plugin", "fallback-key"), Some(b"value".to_vec()));
        assert!(state.delete("plugin", "fallback-key"));
    }

    #[test]
    fn storage_id_separates_plugin_and_key_with_tab() {
        assert_eq!(storage_id("a", "b"), "a\tb");
        assert_eq!(storage_id("a", "b"), storage_id("a", "b"));
    }

    /// 真实 MySQL 往返（需本地服务，默认跳过；v4 同名测试迁移）。
    #[test]
    #[ignore = "requires local MySQL on 127.0.0.1:3306"]
    fn mysql_roundtrip() {
        let config = crate::config::MysqlStorageConfig::default();
        let backend = MysqlStructuredStorage::new(&config).expect("mysql backend");
        roundtrip(&backend);
    }

    /// 真实 MongoDB 往返（需本地服务，默认跳过；v4 同名测试迁移）。
    #[test]
    #[ignore = "requires local MongoDB on 127.0.0.1:27017"]
    fn mongodb_roundtrip() {
        let config = crate::config::MongoStorageConfig {
            username: Some("qexed".to_string()),
            password: Some("qexed".to_string()),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let backend = MongoStructuredStorage::new(&config).expect("mongo backend");
        roundtrip(&backend);
    }
}