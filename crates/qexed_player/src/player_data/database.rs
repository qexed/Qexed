//! 数据库玩家数据后端（v4 player_data/database.rs 迁移）。
//!
//! - MongoPlayerDataStore：payload JSON + raw_nbt Binary 的 update_one upsert，
//!   跨进程序号锁用 `{collection}_locks` 租约文档（owner + expires_at）实现。
//! - MysqlPlayerDataStore：GET_LOCK / RELEASE_LOCK 会话锁 + upsert SQL，
//!   建表时自动补 raw_nbt 列（v4 兼容迁移）。
//!
//! v6 用手动 Box::pin 的 trait 签名（mod.rs 的 PlayerDataStore），迁移时把
//! v4 的 #[async_trait] 方法改写成等价的 Box::pin(async move) 形式。

use std::time::Duration;

use super::{
    PlayerData, PlayerDataStore, PlayerDataStoreLock,
    model::structured_payload,
};
use crate::error::{PlayerError, Result};
use mongodb::bson::doc;

/// 连接失败的告警文案键（用户可见文本一律走 qexed_language::t + .replace()）。
pub(super) const STORAGE_UNAVAILABLE_KEY: &str = "qexed.player.data.storage_unavailable";

const PLAYER_DATA_LOCK_TIMEOUT_SECS: u64 = 30;
const PLAYER_DATA_LOCK_LEASE_SECS: i64 = 60;
const PLAYER_DATA_LOCK_REFRESH_SECS: u64 = 20;
const LOCK_POLL_INTERVAL_MS: u64 = 250;

#[derive(Debug)]
pub(super) struct MongoPlayerDataStore {
    collection: mongodb::Collection<mongodb::bson::Document>,
    locks: mongodb::Collection<mongodb::bson::Document>,
    owner_id: String,
}

impl MongoPlayerDataStore {
    pub(super) async fn new(
        config: &crate::config::MongoConfig,
        collection: &str,
    ) -> Result<Self> {
        config
            .validate()
            .map_err(storage_err)?;
        let client = mongodb::Client::with_uri_str(config.connection_uri())
            .await
            .map_err(storage_err)?;
        let database = client.database(&config.database);
        Ok(Self {
            collection: database.collection(collection),
            locks: database.collection(&format!("{collection}_locks")),
            owner_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    /// 尝试原子抢占租约锁文档；返回 true 表示持有（含续约/抢占过期租约）。
    async fn try_acquire_lock(&self, key: &str, now: mongodb::bson::DateTime) -> bool {
        let expires_at = mongodb::bson::DateTime::from_millis(
            now.timestamp_millis() + PLAYER_DATA_LOCK_LEASE_SECS * 1000,
        );
        let filter = doc! {
            "_id": key,
            "$or": [
                { "owner": &self.owner_id },
                { "expires_at": { "$lte": now } },
            ],
        };
        let update = doc! {
            "$set": {
                "owner": &self.owner_id,
                "expires_at": expires_at,
                "updated_at": now,
            }
        };
        // 与 v4 相同：无唯一索引时并发抢占靠 update_one 的原子性 +
        // duplicate key 容错（建了唯一索引后 E11000 也会被吞掉重试）。
        match self.locks.update_one(filter, update).upsert(true).await {
            Ok(result) => result.modified_count > 0 || result.upserted_id.is_some(),
            Err(err) if duplicate_key_error(&err) => false,
            Err(_) => false,
        }
    }
}

impl PlayerDataStore for MongoPlayerDataStore {
    fn lock(
        &self,
        uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<Box<dyn PlayerDataStoreLock>>>> + Send + '_>>
    {
        Box::pin(async move {
            let key = player_data_lock_key(uuid);
            let deadline = tokio::time::Instant::now()
                + Duration::from_secs(PLAYER_DATA_LOCK_TIMEOUT_SECS);
            loop {
                if self
                    .try_acquire_lock(&key, mongodb::bson::DateTime::now())
                    .await
                {
                    let guard: Box<dyn PlayerDataStoreLock> =
                        Box::new(MongoPlayerDataLock::new(
                            self.locks.clone(),
                            key,
                            self.owner_id.clone(),
                        ));
                    return Ok(Some(guard));
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(PlayerError::msg(format!(
                        "timed out acquiring MongoDB player data lock for uuid={uuid}"
                    )));
                }
                tokio::time::sleep(Duration::from_millis(LOCK_POLL_INTERVAL_MS)).await;
            }
        })
    }

    fn load(
        &self,
        uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<PlayerData>>> + Send + '_>> {
        Box::pin(async move {
            let filter = doc! { "_id": uuid.to_string() };
            let Some(document) = self.collection.find_one(filter).await.map_err(storage_err)? else {
                return Ok(None);
            };
            let payload = document
                .get_str("payload")
                .map_err(|err| PlayerError::msg(format!("MongoDB player data document missing payload field: {err}")))?;
            let mut data: PlayerData = serde_json::from_str(payload)
                .map_err(|err| PlayerError::msg(format!("parse MongoDB player data payload: {err}")))?;
            if let Ok(raw_nbt) = document.get_binary_generic("raw_nbt") {
                data.set_raw_nbt_bytes(raw_nbt);
            }
            Ok(Some(data))
        })
    }

    fn save(
        &self,
        data: &PlayerData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let data = data.clone();
        Box::pin(async move {
            let payload = serde_json::to_string(&structured_payload(&data))?;
            let raw_nbt = data.raw_nbt_bytes()?.unwrap_or_default();
            let filter = doc! { "_id": data.uuid.to_string() };
            let update = doc! {
                "$set": {
                    "uuid": data.uuid.to_string(),
                    "profile_name": &data.profile_name,
                    "payload": payload,
                    "raw_nbt": mongodb::bson::Binary {
                        subtype: mongodb::bson::spec::BinarySubtype::Generic,
                        bytes: raw_nbt,
                    },
                    "updated_at": mongodb::bson::DateTime::now(),
                }
            };
            self.collection
                .update_one(filter, update)
                .upsert(true)
                .await
                .map_err(storage_err)?;
            Ok(())
        })
    }
}

#[derive(Debug)]
struct MongoPlayerDataLock {
    collection: mongodb::Collection<mongodb::bson::Document>,
    key: String,
    owner_id: String,
    stop: tokio::sync::watch::Sender<bool>,
    refresh: tokio::task::JoinHandle<()>,
}

impl MongoPlayerDataLock {
    fn new(
        collection: mongodb::Collection<mongodb::bson::Document>,
        key: String,
        owner_id: String,
    ) -> Self {
        let (stop, stop_rx) = tokio::sync::watch::channel(false);
        let refresh = tokio::spawn(refresh_mongo_lock(
            collection.clone(),
            key.clone(),
            owner_id.clone(),
            stop_rx,
        ));
        Self {
            collection,
            key,
            owner_id,
            stop,
            refresh,
        }
    }
}

impl PlayerDataStoreLock for MongoPlayerDataLock {}

impl Drop for MongoPlayerDataLock {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.refresh.abort();
        let collection = self.collection.clone();
        let key = self.key.clone();
        let owner_id = self.owner_id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = collection
                    .delete_one(doc! {
                        "_id": key,
                        "owner": owner_id,
                    })
                    .await;
            });
        }
    }
}

async fn refresh_mongo_lock(
    collection: mongodb::Collection<mongodb::bson::Document>,
    key: String,
    owner_id: String,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let mut interval =
        tokio::time::interval(Duration::from_secs(PLAYER_DATA_LOCK_REFRESH_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_ok() && *stop.borrow() {
                    return;
                }
            }
            _ = interval.tick() => {
                let now = mongodb::bson::DateTime::now();
                let expires_at = mongodb::bson::DateTime::from_millis(
                    now.timestamp_millis() + PLAYER_DATA_LOCK_LEASE_SECS * 1000,
                );
                let _ = collection
                    .update_one(
                        doc! {
                            "_id": &key,
                            "owner": &owner_id,
                        },
                        doc! {
                            "$set": {
                                "expires_at": expires_at,
                                "updated_at": now,
                            }
                        },
                    )
                    .await;
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct MysqlPlayerDataStore {
    pool: mysql_async::Pool,
    table: String,
}

impl MysqlPlayerDataStore {
    pub(super) async fn new(
        config: &crate::config::MysqlConfig,
        table: &str,
    ) -> Result<Self> {
        config.validate().map_err(storage_err)?;
        validate_mysql_identifier(table)?;
        let opts = mysql_async::Opts::from_url(&config.connection_string())
            .map_err(storage_err)?;
        let pool = mysql_async::Pool::new(opts);
        let store = Self {
            pool,
            table: table.to_string(),
        };
        store.ensure_table().await?;
        Ok(store)
    }

    async fn ensure_table(&self) -> Result<()> {
        use mysql_async::prelude::Queryable;

        let mut conn = self.pool.get_conn().await.map_err(storage_err)?;
        conn.query_drop(format!(
            "CREATE TABLE IF NOT EXISTS `{}` (
                `uuid` CHAR(36) NOT NULL PRIMARY KEY,
                `profile_name` VARCHAR(64) NOT NULL,
                `payload` JSON NOT NULL,
                `raw_nbt` LONGBLOB NULL,
                `updated_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )",
            self.table
        ))
        .await
        .map_err(storage_err)?;
        // v4 迁移兼容：老表无 raw_nbt 列时补上。
        let column_exists: Option<u8> = conn
            .exec_first(
                "SELECT 1
                 FROM INFORMATION_SCHEMA.COLUMNS
                 WHERE TABLE_SCHEMA = DATABASE()
                   AND TABLE_NAME = ?
                   AND COLUMN_NAME = 'raw_nbt'",
                (&self.table,),
            )
            .await
            .map_err(storage_err)?;
        if column_exists.is_none() {
            conn.query_drop(format!(
                "ALTER TABLE `{}` ADD COLUMN `raw_nbt` LONGBLOB NULL AFTER `payload`",
                self.table
            ))
            .await
            .map_err(storage_err)?;
        }
        Ok(())
    }
}

impl PlayerDataStore for MysqlPlayerDataStore {
    fn lock(
        &self,
        uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<Box<dyn PlayerDataStoreLock>>>> + Send + '_>>
    {
        Box::pin(async move {
            use mysql_async::prelude::Queryable;

            let mut conn = self.pool.get_conn().await.map_err(storage_err)?;
            let key = player_data_lock_key(uuid);
            // mysql_async 0.36 已无 params! 宏，positional 参数用元组。
            let acquired: Option<i32> = conn
                .exec_first(
                    "SELECT GET_LOCK(?, ?)",
                    (&key, PLAYER_DATA_LOCK_TIMEOUT_SECS as i32),
                )
                .await
                .map_err(storage_err)?;
            if acquired.unwrap_or_default() != 1 {
                return Err(PlayerError::msg(format!(
                    "timed out acquiring MySQL player data lock for uuid={uuid}"
                )));
            }
            let guard: Box<dyn PlayerDataStoreLock> = Box::new(MysqlPlayerDataLock {
                conn: Some(conn),
                key,
            });
            Ok(Some(guard))
        })
    }

    fn load(
        &self,
        uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<PlayerData>>> + Send + '_>> {
        Box::pin(async move {
            use mysql_async::prelude::Queryable;

            let mut conn = self.pool.get_conn().await.map_err(storage_err)?;
            let uuid = uuid.to_string();
            let row: Option<(String, Option<Vec<u8>>)> = conn
                .exec_first(
                    format!(
                        "SELECT `payload`, `raw_nbt` FROM `{}` WHERE `uuid` = ?",
                        self.table
                    ),
                    (&uuid,),
                )
                .await
                .map_err(storage_err)?;
            row.map(|(payload, raw_nbt)| {
                let mut data: PlayerData = serde_json::from_str(&payload)
                    .map_err(|err| PlayerError::msg(format!("parse MySQL player data payload: {err}")))?;
                if let Some(raw_nbt) = raw_nbt {
                    data.set_raw_nbt_bytes(&raw_nbt);
                }
                Ok(data)
            })
            .transpose()
        })
    }

    fn save(
        &self,
        data: &PlayerData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let data = data.clone();
        Box::pin(async move {
            use mysql_async::prelude::Queryable;

            let payload = serde_json::to_string(&structured_payload(&data))?;
            let raw_nbt = data.raw_nbt_bytes()?.unwrap_or_default();
            let mut conn = self.pool.get_conn().await.map_err(storage_err)?;
            conn.exec_drop(
                format!(
                    "INSERT INTO `{}` (`uuid`, `profile_name`, `payload`, `raw_nbt`)
                     VALUES (?, ?, ?, ?)
                     ON DUPLICATE KEY UPDATE
                        `profile_name` = VALUES(`profile_name`),
                        `payload` = VALUES(`payload`),
                        `raw_nbt` = VALUES(`raw_nbt`)",
                    self.table
                ),
                (
                    data.uuid.to_string(),
                    data.profile_name.clone(),
                    payload,
                    raw_nbt,
                ),
            )
            .await
            .map_err(storage_err)?;
            Ok(())
        })
    }
}

#[derive(Debug)]
struct MysqlPlayerDataLock {
    conn: Option<mysql_async::Conn>,
    key: String,
}

impl PlayerDataStoreLock for MysqlPlayerDataLock {}

impl Drop for MysqlPlayerDataLock {
    fn drop(&mut self) {
        let Some(mut conn) = self.conn.take() else {
            return;
        };
        let key = self.key.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                use mysql_async::prelude::Queryable;
                let _ = conn.exec_drop("SELECT RELEASE_LOCK(?)", (&key,)).await;
            });
        }
    }
}

fn player_data_lock_key(uuid: uuid::Uuid) -> String {
    format!("qexed:player_data:{uuid}")
}

/// 任意驱动错误 → PlayerError::Message（驱动错误不实现 Into<String>，统一走 Display）。
fn storage_err(err: impl std::fmt::Display) -> PlayerError {
    PlayerError::msg(err.to_string())
}
fn duplicate_key_error(err: &mongodb::error::Error) -> bool {
    err.to_string().contains("E11000") || err.to_string().contains("duplicate key")
}

/// 纯逻辑校验：mysql 表名只允许 ASCII 字母/数字/下划线（防 SQL 注入）。
pub(super) fn validate_mysql_identifier(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(PlayerError::msg(
            "MySQL player data table name may only contain ASCII letters, digits, and underscore",
        ))
    }
}

impl Drop for MysqlPlayerDataStore {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = pool.disconnect().await;
            });
        }
    }
}