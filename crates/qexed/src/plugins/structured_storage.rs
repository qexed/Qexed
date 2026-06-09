use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{PluginStructuredStorage, PluginStructuredStorageEngine};

#[derive(Debug)]
pub(super) struct StructuredStorageState {
    backend: Arc<dyn StructuredStorageBackend>,
}

impl Default for StructuredStorageState {
    fn default() -> Self {
        Self {
            backend: Arc::new(SqliteStructuredStorage::new_default()),
        }
    }
}

impl StructuredStorageState {
    pub(super) fn configure(&mut self, config: &PluginStructuredStorage) {
        match build_backend(config) {
            Ok(backend) => self.backend = backend,
            Err(err) => {
                log::warn!("plugin structured storage unavailable: {err:#}; using sqlite");
                self.backend = Arc::new(SqliteStructuredStorage::new_default());
            }
        }
    }

    pub(super) fn exists(&self, plugin: &str, key: &str) -> bool {
        self.backend.exists(plugin, key).unwrap_or(false)
    }

    pub(super) fn get(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        self.backend.get(plugin, key).ok().flatten()
    }

    pub(super) fn set(&self, plugin: &str, key: &str, value: &[u8]) -> bool {
        self.backend.set(plugin, key, value).is_ok()
    }

    pub(super) fn delete(&self, plugin: &str, key: &str) -> bool {
        self.backend.delete(plugin, key).is_ok()
    }
}

fn build_backend(config: &PluginStructuredStorage) -> Result<Arc<dyn StructuredStorageBackend>> {
    match config.engine {
        PluginStructuredStorageEngine::Sqlite => {
            Ok(Arc::new(SqliteStructuredStorage::new_default()))
        }
        PluginStructuredStorageEngine::Mysql => {
            Ok(Arc::new(MysqlStructuredStorage::new(&config.mysql)?))
        }
        PluginStructuredStorageEngine::Mongodb => {
            Ok(Arc::new(MongoStructuredStorage::new(&config.mongodb)?))
        }
    }
}

trait StructuredStorageBackend: Send + Sync + std::fmt::Debug {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool>;
    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>>;
    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<()>;
    fn delete(&self, plugin: &str, key: &str) -> Result<()>;
}

#[derive(Debug)]
struct SqliteStructuredStorage {
    path: PathBuf,
    lock: Mutex<()>,
}

impl SqliteStructuredStorage {
    fn new_default() -> Self {
        Self {
            path: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("config")
                .join("plugin_storage.sqlite3"),
            lock: Mutex::new(()),
        }
    }

    fn connect(&self) -> Result<rusqlite::Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = rusqlite::Connection::open(&self.path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS plugin_structured_storage (
                plugin TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB NOT NULL,
                updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                PRIMARY KEY (plugin, key)
            );",
        )?;
        Ok(conn)
    }
}

impl StructuredStorageBackend for SqliteStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool> {
        let _guard = self
            .lock
            .lock()
            .expect("sqlite plugin storage lock poisoned");
        let conn = self.connect()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plugin_structured_storage WHERE plugin = ?1 AND key = ?2",
            (plugin, key),
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let _guard = self
            .lock
            .lock()
            .expect("sqlite plugin storage lock poisoned");
        let conn = self.connect()?;
        match conn.query_row(
            "SELECT value FROM plugin_structured_storage WHERE plugin = ?1 AND key = ?2",
            (plugin, key),
            |row| row.get(0),
        ) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .expect("sqlite plugin storage lock poisoned");
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO plugin_structured_storage (plugin, key, value, updated_at)
             VALUES (?1, ?2, ?3, unixepoch())
             ON CONFLICT(plugin, key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            (plugin, key, value),
        )?;
        Ok(())
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .expect("sqlite plugin storage lock poisoned");
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM plugin_structured_storage WHERE plugin = ?1 AND key = ?2",
            (plugin, key),
        )?;
        Ok(())
    }
}

#[derive(Debug)]
struct MysqlStructuredStorage {
    runtime: Mutex<tokio::runtime::Runtime>,
    pool: mysql_async::Pool,
}

impl MysqlStructuredStorage {
    fn new(config: &qexed_config::public::mysql::MysqlConfig) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let opts = mysql_opts(config);
        let pool = runtime.block_on(async {
            let pool = mysql_async::Pool::new(opts);
            ensure_mysql_table(&pool).await?;
            Ok::<_, anyhow::Error>(pool)
        })?;
        Ok(Self {
            runtime: Mutex::new(runtime),
            pool,
        })
    }

    fn block_on<T>(&self, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        self.runtime
            .lock()
            .expect("mysql plugin storage runtime poisoned")
            .block_on(future)
    }
}

fn mysql_opts(config: &qexed_config::public::mysql::MysqlConfig) -> mysql_async::Opts {
    mysql_async::Opts::from(
        mysql_async::OptsBuilder::default()
            .ip_or_hostname(config.ip.clone())
            .tcp_port(config.port)
            .user(Some(config.username.clone()))
            .pass(Some(config.password.clone()))
            .db_name(Some(config.database.clone())),
    )
}

async fn ensure_mysql_table(pool: &mysql_async::Pool) -> Result<()> {
    use mysql_async::prelude::Queryable;
    let mut conn = pool.get_conn().await?;
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `qexed_plugin_structured_storage` (
            `plugin` VARCHAR(128) NOT NULL,
            `storage_key` VARCHAR(512) NOT NULL,
            `value` LONGBLOB NOT NULL,
            `updated_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (`plugin`, `storage_key`)
        )",
    )
    .await?;
    Ok(())
}

impl StructuredStorageBackend for MysqlStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool> {
        use mysql_async::{params, prelude::Queryable};
        let plugin = plugin.to_string();
        let key = key.to_string();
        self.block_on(async {
            let mut conn = self.pool.get_conn().await?;
            let count: Option<i64> = conn
                .exec_first(
                    "SELECT COUNT(*) FROM `qexed_plugin_structured_storage`
                     WHERE `plugin` = :plugin AND `storage_key` = :key",
                    params! { "plugin" => plugin, "key" => key },
                )
                .await?;
            Ok(count.unwrap_or(0) > 0)
        })
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>> {
        use mysql_async::{params, prelude::Queryable};
        let plugin = plugin.to_string();
        let key = key.to_string();
        self.block_on(async {
            let mut conn = self.pool.get_conn().await?;
            Ok(conn
                .exec_first(
                    "SELECT `value` FROM `qexed_plugin_structured_storage`
                     WHERE `plugin` = :plugin AND `storage_key` = :key",
                    params! { "plugin" => plugin, "key" => key },
                )
                .await?)
        })
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<()> {
        use mysql_async::{params, prelude::Queryable};
        let plugin = plugin.to_string();
        let key = key.to_string();
        let value = value.to_vec();
        self.block_on(async {
            let mut conn = self.pool.get_conn().await?;
            conn.exec_drop(
                "INSERT INTO `qexed_plugin_structured_storage` (`plugin`, `storage_key`, `value`)
                 VALUES (:plugin, :key, :value)
                 ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)",
                params! { "plugin" => plugin, "key" => key, "value" => value },
            )
            .await?;
            Ok(())
        })
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<()> {
        use mysql_async::{params, prelude::Queryable};
        let plugin = plugin.to_string();
        let key = key.to_string();
        self.block_on(async {
            let mut conn = self.pool.get_conn().await?;
            conn.exec_drop(
                "DELETE FROM `qexed_plugin_structured_storage`
                 WHERE `plugin` = :plugin AND `storage_key` = :key",
                params! { "plugin" => plugin, "key" => key },
            )
            .await?;
            Ok(())
        })
    }
}

#[derive(Debug)]
struct MongoStructuredStorage {
    runtime: Mutex<tokio::runtime::Runtime>,
    client: mongodb::Client,
    database: String,
}

impl MongoStructuredStorage {
    fn new(config: &qexed_config::public::mongodb::MongoConfig) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let client = runtime.block_on(async { mongo_client(config).await })?;
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

    fn block_on<T>(&self, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        self.runtime
            .lock()
            .expect("mongo plugin storage runtime poisoned")
            .block_on(future)
    }
}

impl StructuredStorageBackend for MongoStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        self.block_on(async {
            Ok(collection
                .count_documents(mongodb::bson::doc! { "_id": id })
                .await?
                > 0)
        })
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        self.block_on(async {
            let Some(doc) = collection
                .find_one(mongodb::bson::doc! { "_id": id })
                .await?
            else {
                return Ok(None);
            };
            let value = doc
                .get_binary_generic("value")
                .context("Mongo plugin storage document missing binary value")?
                .to_vec();
            Ok(Some(value))
        })
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<()> {
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
                .await?;
            Ok(())
        })
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<()> {
        let collection = self.collection();
        let id = storage_id(plugin, key);
        self.block_on(async {
            collection
                .delete_one(mongodb::bson::doc! { "_id": id })
                .await?;
            Ok(())
        })
    }
}

async fn mongo_client(
    config: &qexed_config::public::mongodb::MongoConfig,
) -> Result<mongodb::Client> {
    let mut options = mongodb::options::ClientOptions::parse(mongo_connection_uri(config)).await?;
    options.app_name = config.app_name.clone();
    options.connect_timeout = Some(config.connect_timeout);
    options.max_pool_size = Some(config.max_pool_size);
    options.min_pool_size = Some(config.min_pool_size);
    options.max_idle_time = config.max_idle_time;
    if let Some(auth_source) = &config.auth_source {
        if let Some(credential) = &mut options.credential {
            credential.source = Some(auth_source.clone());
        }
    }
    Ok(mongodb::Client::with_options(options)?)
}

fn mongo_connection_uri(config: &qexed_config::public::mongodb::MongoConfig) -> String {
    let protocol = if config.use_tls {
        "mongodb+srv"
    } else {
        "mongodb"
    };
    let mut uri = format!("{protocol}://");
    if let (Some(user), Some(pass)) = (&config.username, &config.password) {
        uri.push_str(user);
        uri.push(':');
        uri.push_str(pass);
        uri.push('@');
    }
    uri.push_str(&format!(
        "{}:{}/{}",
        config.host, config.port, config.database
    ));
    if let Some(auth_source) = &config.auth_source {
        uri.push_str(&format!("?authSource={auth_source}"));
    }
    uri
}

fn storage_id(plugin: &str, key: &str) -> String {
    format!("{plugin}\t{key}")
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
        let value = b"\x00typed-postcard-like-value\xff";
        let updated = b"updated-value";

        assert!(!backend.exists(plugin, &key).expect("exists before set"));
        assert_eq!(backend.get(plugin, &key).expect("get before set"), None);

        backend.set(plugin, &key, value).expect("set value");
        assert!(backend.exists(plugin, &key).expect("exists after set"));
        assert_eq!(
            backend.get(plugin, &key).expect("get after set").as_deref(),
            Some(value.as_slice())
        );

        backend.set(plugin, &key, updated).expect("update value");
        assert_eq!(
            backend
                .get(plugin, &key)
                .expect("get after update")
                .as_deref(),
            Some(updated.as_slice())
        );

        backend.delete(plugin, &key).expect("delete value");
        assert!(!backend.exists(plugin, &key).expect("exists after delete"));
        assert_eq!(backend.get(plugin, &key).expect("get after delete"), None);
    }

    #[test]
    fn sqlite_roundtrip() {
        let backend = SqliteStructuredStorage {
            path: tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("plugin_storage.sqlite3"),
            lock: Mutex::new(()),
        };
        roundtrip(&backend);
    }

    #[test]
    #[ignore = "requires docker/player-data MySQL"]
    fn mysql_roundtrip() {
        let config = qexed_config::public::mysql::MysqlConfig {
            ip: "127.0.0.1".to_string(),
            port: 13306,
            username: "qexed".to_string(),
            password: "qexed".to_string(),
            database: "qexed".to_string(),
            ..Default::default()
        };
        let backend = MysqlStructuredStorage::new(&config).expect("mysql backend");
        roundtrip(&backend);
    }

    #[test]
    #[ignore = "requires docker/player-data MongoDB"]
    fn mongodb_roundtrip() {
        let config = qexed_config::public::mongodb::MongoConfig {
            host: "127.0.0.1".to_string(),
            port: 27017,
            username: Some("qexed".to_string()),
            password: Some("qexed".to_string()),
            database: "qexed".to_string(),
            app_name: Some("qexed-test".to_string()),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let backend = MongoStructuredStorage::new(&config).expect("mongo backend");
        roundtrip(&backend);
    }
}
