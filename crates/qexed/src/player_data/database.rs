use anyhow::{Context, Result};
use async_trait::async_trait;

use super::{PlayerData, PlayerDataStore};
#[derive(Debug)]
pub(super) struct MongoPlayerDataStore {
    collection: mongodb::Collection<mongodb::bson::Document>,
}

impl MongoPlayerDataStore {
    pub(super) async fn new(
        config: &qexed_config::public::mongodb::MongoConfig,
        collection: &str,
    ) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let client = mongodb::Client::with_uri_str(config.connection_uri()).await?;
        Ok(Self {
            collection: client.database(&config.database).collection(collection),
        })
    }
}

#[async_trait]
impl PlayerDataStore for MongoPlayerDataStore {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        let filter = mongodb::bson::doc! { "_id": uuid.to_string() };
        let Some(doc) = self.collection.find_one(filter).await? else {
            return Ok(None);
        };
        let payload = doc
            .get_str("payload")
            .context("MongoDB player data document missing payload field")?;
        serde_json::from_str(payload)
            .map(Some)
            .context("parse MongoDB player data payload")
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        let payload = serde_json::to_string(data)?;
        let filter = mongodb::bson::doc! { "_id": data.uuid.to_string() };
        let update = mongodb::bson::doc! {
            "$set": {
                "uuid": data.uuid.to_string(),
                "profile_name": &data.profile_name,
                "payload": payload,
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
        use mysql_async::prelude::Queryable;

        let mut conn = self.pool.get_conn().await?;
        conn.query_drop(format!(
            "CREATE TABLE IF NOT EXISTS `{}` (
                `uuid` CHAR(36) NOT NULL PRIMARY KEY,
                `profile_name` VARCHAR(64) NOT NULL,
                `payload` JSON NOT NULL,
                `updated_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )",
            self.table
        ))
        .await?;
        Ok(())
    }
}

#[async_trait]
impl PlayerDataStore for MysqlPlayerDataStore {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let payload: Option<String> = conn
            .exec_first(
                format!(
                    "SELECT `payload` FROM `{}` WHERE `uuid` = :uuid",
                    self.table
                ),
                params! { "uuid" => uuid.to_string() },
            )
            .await?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).context("parse MySQL player data payload")
            })
            .transpose()
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        use mysql_async::{params, prelude::Queryable};

        let payload = serde_json::to_string(data)?;
        let mut conn = self.pool.get_conn().await?;
        conn.exec_drop(
            format!(
                "INSERT INTO `{}` (`uuid`, `profile_name`, `payload`)
                 VALUES (:uuid, :profile_name, :payload)
                 ON DUPLICATE KEY UPDATE
                    `profile_name` = VALUES(`profile_name`),
                    `payload` = VALUES(`payload`)",
                self.table
            ),
            params! {
                "uuid" => data.uuid.to_string(),
                "profile_name" => &data.profile_name,
                "payload" => payload,
            },
        )
        .await?;
        Ok(())
    }
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
