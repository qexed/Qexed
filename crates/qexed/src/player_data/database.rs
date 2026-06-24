use std::time::Duration;

use super::{PlayerData, PlayerDataStore, PlayerDataStoreLock};
use anyhow::{Context, Result};
use async_trait::async_trait;
use uuid::Uuid;

const PLAYER_DATA_LOCK_TIMEOUT_SECS: u64 = 30;
const PLAYER_DATA_LOCK_LEASE_SECS: i64 = 60;
const PLAYER_DATA_LOCK_REFRESH_SECS: u64 = 20;

#[derive(Debug)]
pub(super) struct MongoPlayerDataStore {
    collection: mongodb::Collection<mongodb::bson::Document>,
    locks: mongodb::Collection<mongodb::bson::Document>,
    owner_id: String,
}

impl MongoPlayerDataStore {
    pub(super) async fn new(
        config: &qexed_config::public::mongodb::MongoConfig,
        collection: &str,
    ) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let client = mongodb::Client::with_uri_str(config.connection_uri()).await?;
        let database = client.database(&config.database);
        Ok(Self {
            collection: database.collection(collection),
            locks: database.collection(&format!("{collection}_locks")),
            owner_id: Uuid::new_v4().to_string(),
        })
    }
}

#[async_trait]
impl PlayerDataStore for MongoPlayerDataStore {
    async fn lock(&self, uuid: uuid::Uuid) -> Result<Option<Box<dyn PlayerDataStoreLock>>> {
        let key = player_data_lock_key(uuid);
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(PLAYER_DATA_LOCK_TIMEOUT_SECS);
        loop {
            let now = mongodb::bson::DateTime::now();
            let expires_at = mongodb::bson::DateTime::from_millis(
                now.timestamp_millis() + PLAYER_DATA_LOCK_LEASE_SECS * 1000,
            );
            let filter = mongodb::bson::doc! {
                "_id": &key,
                "$or": [
                    { "owner": &self.owner_id },
                    { "expires_at": { "$lte": now } },
                ],
            };
            let update = mongodb::bson::doc! {
                "$set": {
                    "owner": &self.owner_id,
                    "expires_at": expires_at,
                    "updated_at": now,
                }
            };
            let result = self.locks.update_one(filter, update).upsert(true).await;
            match result {
                Ok(result) if result.modified_count > 0 || result.upserted_id.is_some() => {
                    return Ok(Some(Box::new(MongoPlayerDataLock::new(
                        self.locks.clone(),
                        key,
                        self.owner_id.clone(),
                    ))));
                }
                Ok(_) => {}
                Err(err) if duplicate_key_error(&err) => {}
                Err(err) => return Err(err.into()),
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("timed out acquiring MongoDB player data lock for uuid={uuid}");
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        let filter = mongodb::bson::doc! { "_id": uuid.to_string() };
        let Some(doc) = self.collection.find_one(filter).await? else {
            return Ok(None);
        };
        let payload = doc
            .get_str("payload")
            .context("MongoDB player data document missing payload field")?;
        let mut data: PlayerData =
            serde_json::from_str(payload).context("parse MongoDB player data payload")?;
        if let Ok(raw_nbt) = doc.get_binary_generic("raw_nbt") {
            data.set_raw_nbt_bytes(raw_nbt);
        }
        Ok(Some(data))
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        let payload = serde_json::to_string(&structured_payload(data))?;
        let raw_nbt = data.raw_nbt_bytes()?.unwrap_or_default();
        let filter = mongodb::bson::doc! { "_id": data.uuid.to_string() };
        let update = mongodb::bson::doc! {
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
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct MysqlPlayerDataStore {
    pool: mysql_async::Pool,
    table: String,
}

impl MysqlPlayerDataStore {
    pub(super) async fn new(
        config: &qexed_config::public::mysql::MysqlConfig,
        table: &str,
    ) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        validate_mysql_identifier(table)?;
        let opts = mysql_async::Opts::from_url(&config.connection_string())?;
        let pool = mysql_async::Pool::new(opts);
        let store = Self {
            pool,
            table: table.to_string(),
        };
        store.ensure_table().await?;
        Ok(store)
    }

    async fn ensure_table(&self) -> Result<()> {
        use mysql_async::params;
        use mysql_async::prelude::Queryable;

        let mut conn = self.pool.get_conn().await?;
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
        .await?;
        let column_exists: Option<u8> = conn
            .exec_first(
                "SELECT 1
                 FROM INFORMATION_SCHEMA.COLUMNS
                 WHERE TABLE_SCHEMA = DATABASE()
                   AND TABLE_NAME = :table
                   AND COLUMN_NAME = 'raw_nbt'",
                params! { "table" => &self.table },
            )
            .await?;
        if column_exists.is_none() {
            conn.query_drop(format!(
                "ALTER TABLE `{}` ADD COLUMN `raw_nbt` LONGBLOB NULL AFTER `payload`",
                self.table
            ))
            .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl PlayerDataStore for MysqlPlayerDataStore {
    async fn lock(&self, uuid: uuid::Uuid) -> Result<Option<Box<dyn PlayerDataStoreLock>>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let key = player_data_lock_key(uuid);
        let acquired: Option<i32> = conn
            .exec_first(
                "SELECT GET_LOCK(:lock_key, :timeout_secs)",
                params! {
                    "lock_key" => &key,
                    "timeout_secs" => PLAYER_DATA_LOCK_TIMEOUT_SECS as i32,
                },
            )
            .await?;
        if acquired.unwrap_or_default() != 1 {
            anyhow::bail!("timed out acquiring MySQL player data lock for uuid={uuid}");
        }
        Ok(Some(Box::new(MysqlPlayerDataLock {
            conn: Some(conn),
            key,
        })))
    }

    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let row: Option<(String, Option<Vec<u8>>)> = conn
            .exec_first(
                format!(
                    "SELECT `payload`, `raw_nbt` FROM `{}` WHERE `uuid` = :uuid",
                    self.table
                ),
                params! { "uuid" => uuid.to_string() },
            )
            .await?;
        row.map(|(payload, raw_nbt)| {
            let mut data: PlayerData =
                serde_json::from_str(&payload).context("parse MySQL player data payload")?;
            if let Some(raw_nbt) = raw_nbt {
                data.set_raw_nbt_bytes(&raw_nbt);
            }
            Ok(data)
        })
        .transpose()
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        use mysql_async::{params, prelude::Queryable};

        let payload = serde_json::to_string(&structured_payload(data))?;
        let raw_nbt = data.raw_nbt_bytes()?.unwrap_or_default();
        let mut conn = self.pool.get_conn().await?;
        conn.exec_drop(
            format!(
                "INSERT INTO `{}` (`uuid`, `profile_name`, `payload`, `raw_nbt`)
                 VALUES (:uuid, :profile_name, :payload, :raw_nbt)
                 ON DUPLICATE KEY UPDATE
                    `profile_name` = VALUES(`profile_name`),
                    `payload` = VALUES(`payload`),
                    `raw_nbt` = VALUES(`raw_nbt`)",
                self.table
            ),
            params! {
                "uuid" => data.uuid.to_string(),
                "profile_name" => &data.profile_name,
                "payload" => payload,
                "raw_nbt" => raw_nbt,
            },
        )
        .await?;
        Ok(())
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
                use mysql_async::{params, prelude::Queryable};
                let _ = conn
                    .exec_drop(
                        "SELECT RELEASE_LOCK(:lock_key)",
                        params! { "lock_key" => key },
                    )
                    .await;
            });
        }
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
                    .delete_one(mongodb::bson::doc! {
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
    let mut interval = tokio::time::interval(Duration::from_secs(PLAYER_DATA_LOCK_REFRESH_SECS));
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
                        mongodb::bson::doc! {
                            "_id": &key,
                            "owner": &owner_id,
                        },
                        mongodb::bson::doc! {
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

fn player_data_lock_key(uuid: uuid::Uuid) -> String {
    format!("qexed:player_data:{uuid}")
}

fn duplicate_key_error(err: &mongodb::error::Error) -> bool {
    err.to_string().contains("E11000") || err.to_string().contains("duplicate key")
}

fn structured_payload(data: &PlayerData) -> PlayerData {
    let mut payload = data.clone();
    payload.raw_nbt.clear();
    payload
}

pub(super) fn validate_mysql_identifier(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        anyhow::bail!(
            "MySQL player data table name may only contain ASCII letters, digits, and underscore"
        )
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
