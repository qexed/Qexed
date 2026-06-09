use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use qexed_config::app::qexed::server::{Economy, EconomyStorageEngine};

pub(super) const DEFAULT_CURRENCY: &str = "qexed:coin";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct CurrencyInfo {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) symbol: String,
    pub(super) fractional_digits: i32,
    #[serde(default = "default_economy_storage")]
    pub(super) storage: String,
}

#[derive(Debug)]
pub(super) struct EconomyState {
    currencies: BTreeMap<String, CurrencyInfo>,
    stores: HashMap<&'static str, Arc<dyn EconomyStore>>,
}

impl Default for EconomyState {
    fn default() -> Self {
        let mut state = Self {
            currencies: BTreeMap::new(),
            stores: HashMap::new(),
        };
        state.ensure_default_currency();
        state.install_store("sqlite", Arc::new(SqliteEconomyStore::new_default()));
        state
    }
}

impl EconomyState {
    pub(super) fn configure(&mut self, config: &Economy) {
        self.currencies.clear();
        for currency in &config.currencies {
            let id = normalize_currency(&currency.id);
            if id.is_empty() {
                continue;
            }
            self.currencies.insert(
                id.clone(),
                CurrencyInfo {
                    id,
                    name: currency.name.clone(),
                    symbol: currency.symbol.clone(),
                    fractional_digits: currency.fractional_digits.clamp(0, 8),
                    storage: economy_storage_name(currency.storage).to_string(),
                },
            );
        }
        self.ensure_default_currency();
        self.configure_store("sqlite", || Ok(Arc::new(SqliteEconomyStore::new_default())));
        if self.uses_storage(EconomyStorageEngine::Mysql) {
            let config = config.mysql.clone();
            self.configure_store("mysql", move || {
                block_on(MysqlEconomyStore::new(&config)).map(|store| Arc::new(store) as _)
            });
        }
        if self.uses_storage(EconomyStorageEngine::Mongodb) {
            let config = config.mongodb.clone();
            self.configure_store("mongodb", move || {
                block_on(MongoEconomyStore::new(&config)).map(|store| Arc::new(store) as _)
            });
        }
        if self.uses_storage(EconomyStorageEngine::Redis) {
            let config = config.redis.clone();
            self.configure_store("redis", move || {
                block_on(RedisEconomyStore::new(&config)).map(|store| Arc::new(store) as _)
            });
        }
    }

    pub(super) fn register_currency(
        &mut self,
        id: String,
        name: String,
        symbol: String,
        fractional_digits: i32,
    ) {
        self.currencies.insert(
            id.clone(),
            CurrencyInfo {
                id,
                name,
                symbol,
                fractional_digits: fractional_digits.clamp(0, 8),
                storage: default_economy_storage(),
            },
        );
    }

    pub(super) fn currency_info(&self, currency: &str) -> Option<CurrencyInfo> {
        let currency = normalize_currency(currency);
        self.currencies
            .get(if currency.is_empty() {
                DEFAULT_CURRENCY
            } else {
                &currency
            })
            .cloned()
    }

    pub(super) fn normalize_existing_currency(&self, currency: &str) -> Option<String> {
        let currency = normalize_currency(currency);
        let currency = if currency.is_empty() {
            DEFAULT_CURRENCY.to_string()
        } else {
            currency
        };
        self.currencies.contains_key(&currency).then_some(currency)
    }

    pub(super) fn storage_for(&self, currency: &str) -> String {
        self.currency_info(currency)
            .map(|currency| currency.storage)
            .unwrap_or_else(default_economy_storage)
    }

    pub(super) fn balance(&self, player: &str, currency: &str) -> Option<i64> {
        let store = self.store_for_currency(currency)?;
        block_on(store.balance(player, currency)).ok()
    }

    pub(super) fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        let store = self.store_for_currency(currency)?;
        block_on(store.set_balance(player, currency, amount)).ok()
    }

    pub(super) fn deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        let store = self.store_for_currency(currency)?;
        block_on(store.deposit(player, currency, amount)).ok()
    }

    pub(super) fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        let store = self.store_for_currency(currency)?;
        block_on(store.withdraw(player, currency, amount))
            .ok()
            .flatten()
    }

    fn ensure_default_currency(&mut self) {
        self.currencies
            .entry(DEFAULT_CURRENCY.to_string())
            .or_insert_with(|| CurrencyInfo {
                id: DEFAULT_CURRENCY.to_string(),
                name: "Coin".to_string(),
                symbol: "Q".to_string(),
                fractional_digits: 2,
                storage: default_economy_storage(),
            });
    }

    fn uses_storage(&self, storage: EconomyStorageEngine) -> bool {
        let storage = economy_storage_name(storage);
        self.currencies
            .values()
            .any(|currency| currency.storage == storage)
    }

    fn configure_store(
        &mut self,
        name: &'static str,
        build: impl FnOnce() -> Result<Arc<dyn EconomyStore>>,
    ) {
        match build() {
            Ok(store) => self.install_store(name, store),
            Err(err) => log::warn!("plugin economy {name} store unavailable: {err:#}"),
        }
    }

    fn install_store(&mut self, name: &'static str, store: Arc<dyn EconomyStore>) {
        self.stores.insert(name, store);
    }

    fn store_for_currency(&self, currency: &str) -> Option<Arc<dyn EconomyStore>> {
        let storage = self.storage_for(currency);
        self.stores.get(storage.as_str()).cloned()
    }
}

#[async_trait]
trait EconomyStore: Send + Sync + std::fmt::Debug {
    async fn balance(&self, player: &str, currency: &str) -> Result<i64>;
    async fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Result<i64>;
    async fn deposit(&self, player: &str, currency: &str, amount: i64) -> Result<i64>;
    async fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Result<Option<i64>>;
}

#[derive(Debug)]
struct SqliteEconomyStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl SqliteEconomyStore {
    fn new_default() -> Self {
        Self {
            path: economy_sqlite_path(),
            lock: Mutex::new(()),
        }
    }

    fn connect(&self) -> Result<rusqlite::Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = rusqlite::Connection::open(&self.path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS economy_balances (
                player TEXT NOT NULL,
                currency TEXT NOT NULL,
                amount INTEGER NOT NULL,
                PRIMARY KEY (player, currency)
            );",
        )?;
        Ok(conn)
    }
}

#[async_trait]
impl EconomyStore for SqliteEconomyStore {
    async fn balance(&self, player: &str, currency: &str) -> Result<i64> {
        let _guard = self.lock.lock().expect("sqlite economy lock poisoned");
        let conn = self.connect()?;
        let amount = conn
            .query_row(
                "SELECT amount FROM economy_balances WHERE player = ?1 AND currency = ?2",
                (player, currency),
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(amount)
    }

    async fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        let _guard = self.lock.lock().expect("sqlite economy lock poisoned");
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO economy_balances (player, currency, amount)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(player, currency) DO UPDATE SET amount = excluded.amount",
            (player, currency, amount),
        )?;
        Ok(amount)
    }

    async fn deposit(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        let _guard = self.lock.lock().expect("sqlite economy lock poisoned");
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO economy_balances (player, currency, amount)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(player, currency) DO UPDATE SET amount = amount + excluded.amount",
            (player, currency, amount),
        )?;
        self.balance_without_lock(&conn, player, currency)
    }

    async fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Result<Option<i64>> {
        let _guard = self.lock.lock().expect("sqlite economy lock poisoned");
        let conn = self.connect()?;
        let changed = conn.execute(
            "UPDATE economy_balances
             SET amount = amount - ?3
             WHERE player = ?1 AND currency = ?2 AND amount >= ?3",
            (player, currency, amount),
        )?;
        if changed == 0 {
            return Ok(None);
        }
        self.balance_without_lock(&conn, player, currency).map(Some)
    }
}

impl SqliteEconomyStore {
    fn balance_without_lock(
        &self,
        conn: &rusqlite::Connection,
        player: &str,
        currency: &str,
    ) -> Result<i64> {
        Ok(conn
            .query_row(
                "SELECT amount FROM economy_balances WHERE player = ?1 AND currency = ?2",
                (player, currency),
                |row| row.get(0),
            )
            .unwrap_or(0))
    }
}

#[derive(Debug)]
struct MysqlEconomyStore {
    opts: mysql_async::Opts,
}

impl MysqlEconomyStore {
    async fn new(config: &qexed_config::public::mysql::MysqlConfig) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let opts = mysql_async::Opts::from(
            mysql_async::OptsBuilder::default()
                .ip_or_hostname(config.ip.clone())
                .tcp_port(config.port)
                .user(Some(config.username.clone()))
                .pass(Some(config.password.clone()))
                .db_name(Some(config.database.clone())),
        );
        let store = Self { opts };
        store.ensure_table().await?;
        Ok(store)
    }

    async fn conn(&self) -> Result<mysql_async::Conn> {
        Ok(mysql_async::Conn::new(self.opts.clone()).await?)
    }

    async fn ensure_table(&self) -> Result<()> {
        use mysql_async::prelude::Queryable;

        let mut conn = self.conn().await?;
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS `qexed_economy_balances` (
                `player` VARCHAR(128) NOT NULL,
                `currency` VARCHAR(128) NOT NULL,
                `amount` BIGINT NOT NULL,
                PRIMARY KEY (`player`, `currency`)
            )",
        )
        .await?;
        Ok(())
    }
}

#[async_trait]
impl EconomyStore for MysqlEconomyStore {
    async fn balance(&self, player: &str, currency: &str) -> Result<i64> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.conn().await?;
        Ok(conn
            .exec_first(
                "SELECT `amount` FROM `qexed_economy_balances`
                 WHERE `player` = :player AND `currency` = :currency",
                params! { "player" => player, "currency" => currency },
            )
            .await?
            .unwrap_or(0))
    }

    async fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.conn().await?;
        conn.exec_drop(
            "INSERT INTO `qexed_economy_balances` (`player`, `currency`, `amount`)
             VALUES (:player, :currency, :amount)
             ON DUPLICATE KEY UPDATE `amount` = VALUES(`amount`)",
            params! { "player" => player, "currency" => currency, "amount" => amount },
        )
        .await?;
        Ok(amount)
    }

    async fn deposit(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.conn().await?;
        conn.exec_drop(
            "INSERT INTO `qexed_economy_balances` (`player`, `currency`, `amount`)
             VALUES (:player, :currency, :amount)
             ON DUPLICATE KEY UPDATE `amount` = `amount` + VALUES(`amount`)",
            params! { "player" => player, "currency" => currency, "amount" => amount },
        )
        .await?;
        self.balance(player, currency).await
    }

    async fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Result<Option<i64>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.conn().await?;
        conn.exec_drop(
            "UPDATE `qexed_economy_balances`
             SET `amount` = `amount` - :amount
             WHERE `player` = :player AND `currency` = :currency AND `amount` >= :amount",
            params! { "player" => player, "currency" => currency, "amount" => amount },
        )
        .await?;
        if conn.affected_rows() == 0 {
            return Ok(None);
        }
        self.balance(player, currency).await.map(Some)
    }
}

#[derive(Debug)]
struct MongoEconomyStore {
    config: qexed_config::public::mongodb::MongoConfig,
}

impl MongoEconomyStore {
    async fn new(config: &qexed_config::public::mongodb::MongoConfig) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let store = Self {
            config: config.clone(),
        };
        let _ = store.collection().await?;
        Ok(store)
    }

    async fn collection(&self) -> Result<mongodb::Collection<mongodb::bson::Document>> {
        let config = &self.config;
        let mut uri = if config.use_tls {
            "mongodb+srv://".to_string()
        } else {
            "mongodb://".to_string()
        };
        if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            uri.push_str(&format!("{user}:{pass}@"));
        }
        uri.push_str(&format!(
            "{}:{}/{}",
            config.host, config.port, config.database
        ));
        let mut options = mongodb::options::ClientOptions::parse(uri).await?;
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
        let client = mongodb::Client::with_options(options)?;
        Ok(client
            .database(&config.database)
            .collection("qexed_economy_balances"))
    }
}

#[async_trait]
impl EconomyStore for MongoEconomyStore {
    async fn balance(&self, player: &str, currency: &str) -> Result<i64> {
        let collection = self.collection().await?;
        let filter = mongodb::bson::doc! { "_id": balance_key(player, currency) };
        let Some(doc) = collection.find_one(filter).await? else {
            return Ok(0);
        };
        Ok(doc.get_i64("amount").unwrap_or(0))
    }

    async fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        let collection = self.collection().await?;
        let filter = mongodb::bson::doc! { "_id": balance_key(player, currency) };
        let update = mongodb::bson::doc! {
            "$set": {
                "player": player,
                "currency": currency,
                "amount": amount,
                "updated_at": mongodb::bson::DateTime::now(),
            }
        };
        collection.update_one(filter, update).upsert(true).await?;
        Ok(amount)
    }

    async fn deposit(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        let collection = self.collection().await?;
        let filter = mongodb::bson::doc! { "_id": balance_key(player, currency) };
        let update = mongodb::bson::doc! {
            "$setOnInsert": { "player": player, "currency": currency },
            "$inc": { "amount": amount },
            "$set": { "updated_at": mongodb::bson::DateTime::now() },
        };
        collection.update_one(filter, update).upsert(true).await?;
        self.balance(player, currency).await
    }

    async fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Result<Option<i64>> {
        let collection = self.collection().await?;
        let filter = mongodb::bson::doc! {
            "_id": balance_key(player, currency),
            "amount": { "$gte": amount },
        };
        let update = mongodb::bson::doc! {
            "$inc": { "amount": -amount },
            "$set": { "updated_at": mongodb::bson::DateTime::now() },
        };
        let result = collection.update_one(filter, update).await?;
        if result.modified_count == 0 {
            return Ok(None);
        }
        self.balance(player, currency).await.map(Some)
    }
}

#[derive(Debug)]
struct RedisEconomyStore {
    client: redis::Client,
}

impl RedisEconomyStore {
    async fn new(config: &qexed_config::public::pika::PikaConfig) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        Ok(Self {
            client: redis::Client::open(config.connection_params())?,
        })
    }

    async fn connection(&self) -> Result<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .context("connect Redis economy store")
    }
}

#[async_trait]
impl EconomyStore for RedisEconomyStore {
    async fn balance(&self, player: &str, currency: &str) -> Result<i64> {
        use redis::AsyncCommands;

        let mut conn = self.connection().await?;
        Ok(conn.get(redis_key(player, currency)).await.unwrap_or(0))
    }

    async fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        use redis::AsyncCommands;

        let mut conn = self.connection().await?;
        let _: () = conn.set(redis_key(player, currency), amount).await?;
        Ok(amount)
    }

    async fn deposit(&self, player: &str, currency: &str, amount: i64) -> Result<i64> {
        use redis::AsyncCommands;

        let mut conn = self.connection().await?;
        Ok(conn.incr(redis_key(player, currency), amount).await?)
    }

    async fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Result<Option<i64>> {
        let mut conn = self.connection().await?;
        let key = redis_key(player, currency);
        let script = redis::Script::new(
            "local value = tonumber(redis.call('GET', KEYS[1]) or '0')
             local amount = tonumber(ARGV[1])
             if value < amount then return nil end
             value = value - amount
             redis.call('SET', KEYS[1], value)
             return value",
        );
        Ok(script.key(key).arg(amount).invoke_async(&mut conn).await?)
    }
}

pub(super) fn normalize_currency(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(super) fn default_economy_storage() -> String {
    "sqlite".to_string()
}

pub(super) fn economy_storage_name(storage: EconomyStorageEngine) -> &'static str {
    match storage {
        EconomyStorageEngine::Sqlite => "sqlite",
        EconomyStorageEngine::Mysql => "mysql",
        EconomyStorageEngine::Redis => "redis",
        EconomyStorageEngine::Mongodb => "mongodb",
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("plugin economy runtime build failed")
            .block_on(future)
    }
}

fn economy_sqlite_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config")
        .join("economy.sqlite3")
}

fn balance_key(player: &str, currency: &str) -> String {
    format!("{player}\t{currency}")
}

fn redis_key(player: &str, currency: &str) -> String {
    format!("qexed:economy:{currency}:{player}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: &str = "00000000-0000-0000-0000-000000000001";
    const CURRENCY: &str = "qexed:test";

    #[test]
    fn sqlite_store_roundtrips_balances() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteEconomyStore {
            path: dir.path().join("economy.sqlite3"),
            lock: Mutex::new(()),
        };

        assert_eq!(block_on(store.balance(PLAYER, CURRENCY)).unwrap(), 0);
        assert_eq!(
            block_on(store.set_balance(PLAYER, CURRENCY, 100)).unwrap(),
            100
        );
        assert_eq!(block_on(store.deposit(PLAYER, CURRENCY, 25)).unwrap(), 125);
        assert_eq!(
            block_on(store.withdraw(PLAYER, CURRENCY, 200)).unwrap(),
            None
        );
        assert_eq!(
            block_on(store.withdraw(PLAYER, CURRENCY, 40)).unwrap(),
            Some(85)
        );
        assert_eq!(block_on(store.balance(PLAYER, CURRENCY)).unwrap(), 85);
    }

    #[test]
    fn state_routes_configured_currency_to_storage() {
        let mut state = EconomyState::default();
        let config = Economy {
            currencies: vec![qexed_config::app::qexed::server::EconomyCurrency {
                id: CURRENCY.to_string(),
                storage: EconomyStorageEngine::Mysql,
                ..Default::default()
            }],
            ..Default::default()
        };

        state.configure(&config);

        assert_eq!(state.storage_for(CURRENCY), "mysql");
        assert!(state.set_balance(PLAYER, CURRENCY, 1).is_none());
    }

    #[test]
    #[ignore = "requires local MySQL on 127.0.0.1:13306"]
    fn mysql_store_roundtrips_balances() {
        let config = qexed_config::public::mysql::MysqlConfig {
            port: 13306,
            username: "qexed".to_string(),
            password: "qexed".to_string(),
            database: "qexed".to_string(),
            ..Default::default()
        };
        let store = block_on(MysqlEconomyStore::new(&config)).unwrap();
        let currency = "qexed:mysql_test";

        assert_eq!(
            block_on(store.set_balance(PLAYER, currency, 100)).unwrap(),
            100
        );
        assert_eq!(block_on(store.deposit(PLAYER, currency, 25)).unwrap(), 125);
        assert_eq!(
            block_on(store.withdraw(PLAYER, currency, 200)).unwrap(),
            None
        );
        assert_eq!(
            block_on(store.withdraw(PLAYER, currency, 40)).unwrap(),
            Some(85)
        );
    }

    #[test]
    #[ignore = "requires local MongoDB on 127.0.0.1:27017"]
    fn mongodb_store_roundtrips_balances() {
        let config = qexed_config::public::mongodb::MongoConfig {
            username: Some("qexed".to_string()),
            password: Some("qexed".to_string()),
            database: "qexed".to_string(),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let store = block_on(MongoEconomyStore::new(&config)).unwrap();
        let currency = "qexed:mongodb_test";

        assert_eq!(
            block_on(store.set_balance(PLAYER, currency, 100)).unwrap(),
            100
        );
        assert_eq!(block_on(store.deposit(PLAYER, currency, 25)).unwrap(), 125);
        assert_eq!(
            block_on(store.withdraw(PLAYER, currency, 200)).unwrap(),
            None
        );
        assert_eq!(
            block_on(store.withdraw(PLAYER, currency, 40)).unwrap(),
            Some(85)
        );
    }

    #[test]
    #[ignore = "requires local Redis/Pika on 127.0.0.1:9221"]
    fn redis_store_roundtrips_balances() {
        let config = qexed_config::public::pika::PikaConfig {
            password: None,
            database: 0,
            ..Default::default()
        };
        let store = block_on(RedisEconomyStore::new(&config)).unwrap();
        let currency = "qexed:redis_test";

        assert_eq!(
            block_on(store.set_balance(PLAYER, currency, 100)).unwrap(),
            100
        );
        assert_eq!(block_on(store.deposit(PLAYER, currency, 25)).unwrap(), 125);
        assert_eq!(
            block_on(store.withdraw(PLAYER, currency, 200)).unwrap(),
            None
        );
        assert_eq!(
            block_on(store.withdraw(PLAYER, currency, 40)).unwrap(),
            Some(85)
        );
    }
}
