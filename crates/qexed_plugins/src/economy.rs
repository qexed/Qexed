//! 插件经济系统（v4 plugins/economy.rs 迁移）。
//!
//! 四种存储：sqlite 枚举位由 JSON 文件存储承载（v6 无 rusqlite，
//! config/economy.json，表结构与 v4 sqlite 同构：player+currency -> amount）；
//! mysql / mongodb / redis 为 v4 实现的完整迁移，各挂一个专用 current_thread
//! tokio Runtime 驱动异步客户端，连接失败时槽位不装（查询回退失败，不 panic）。

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::config::{EconomyConfig, EconomyStorageEngine};

pub(crate) const DEFAULT_CURRENCY: &str = "qexed:coin";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct CurrencyInfo {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub fractional_digits: i32,
    #[serde(default = "default_economy_storage")]
    pub storage: String,
}

#[derive(Debug)]
pub(crate) struct EconomyState {
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
        state.install_store("sqlite", Arc::new(FileEconomyStore::new_default()));
        state
    }
}

impl EconomyState {
    pub(crate) fn configure(&mut self, config: &EconomyConfig) {
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
                    storage: currency.storage.as_str().to_string(),
                },
            );
        }
        self.ensure_default_currency();
        // sqlite 槽位用文件实现；mysql/mongodb/redis 按需构建（v4 实现 迁移）。
        self.configure_store("sqlite", || Ok(Arc::new(FileEconomyStore::new_default())));
        if self.uses_storage(EconomyStorageEngine::Mysql) {
            let config = config.mysql.clone();
            self.configure_store("mysql", move || {
                Ok(Arc::new(MysqlEconomyStore::new(&config)?))
            });
        }
        if self.uses_storage(EconomyStorageEngine::Mongodb) {
            let config = config.mongodb.clone();
            self.configure_store("mongodb", move || {
                Ok(Arc::new(MongoEconomyStore::new(&config)?))
            });
        }
        if self.uses_storage(EconomyStorageEngine::Redis) {
            let config = config.redis.clone();
            self.configure_store("redis", move || {
                Ok(Arc::new(RedisEconomyStore::new(&config)?))
            });
        }
    }

    pub(crate) fn register_currency(
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

    pub(crate) fn currency_info(&self, currency: &str) -> Option<CurrencyInfo> {
        let currency = normalize_currency(currency);
        self.currencies
            .get(if currency.is_empty() {
                DEFAULT_CURRENCY
            } else {
                &currency
            })
            .cloned()
    }

    pub(crate) fn normalize_existing_currency(&self, currency: &str) -> Option<String> {
        let currency = normalize_currency(currency);
        let currency = if currency.is_empty() {
            DEFAULT_CURRENCY.to_string()
        } else {
            currency
        };
        self.currencies.contains_key(&currency).then_some(currency)
    }

    pub(crate) fn storage_for(&self, currency: &str) -> String {
        self.currency_info(currency)
            .map(|currency| currency.storage)
            .unwrap_or_else(default_economy_storage)
    }

    pub(crate) fn balance(&self, player: &str, currency: &str) -> Option<i64> {
        let store = self.store_for_currency(currency)?;
        store.balance(player, currency).ok()
    }

    pub(crate) fn set_balance(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        // 值校验（防内存串改落地）：余额非负且在合理上界内。
        if !Self::amount_invariant_ok(amount) {
            log::warn!("economy set_balance rejected: player={player} amount={amount}");
            return None;
        }
        let store = self.store_for_currency(currency)?;
        store.set_balance(player, currency, amount).ok()
    }

    /// 余额不变量：非负 + ≤ 2^62/4（溢出/天价视为篡改痕迹）。
    fn amount_invariant_ok(amount: i64) -> bool {
        (0..=i64::MAX / 4).contains(&amount)
    }

    pub(crate) fn deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        // 存入增量校验：非法增量直接拒绝（防 CE 一次性写天价）。
        if amount < 0 || amount > 1_000_000_000 {
            log::warn!("economy deposit rejected: player={player} amount={amount}");
            return None;
        }
        let store = self.store_for_currency(currency)?;
        store.deposit(player, currency, amount).ok()
    }

    pub(crate) fn withdraw(&self, player: &str, currency: &str, amount: i64) -> Option<Option<i64>> {
        let store = self.store_for_currency(currency)?;
        store.withdraw(player, currency, amount).ok()
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
        let storage = storage.as_str();
        self.currencies
            .values()
            .any(|currency| currency.storage == storage)
    }

    fn configure_store(
        &mut self,
        name: &'static str,
        build: impl FnOnce() -> Result<Arc<dyn EconomyStore>, crate::error::PluginsError>,
    ) {
        match build() {
            Ok(store) => self.install_store(name, store),
            Err(err) => log::warn!(
                "{}",
                qexed_language::t("qexed.plugins.economy.store.unavailable")
                    .replace("%{store}", name)
                    .replace("%{error}", &err.to_string())
            ),
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

/// 经济余额存储接口（文件 / mysql / mongodb / redis 四实现）。
///
/// v4 为 async_trait；v6 保持同步签名（宿主服务在插件线程上调用），
/// 数据库实现内部用专用 Runtime block_on。
pub(crate) trait EconomyStore: Send + Sync + std::fmt::Debug {
    fn balance(&self, player: &str, currency: &str) -> Result<i64, crate::error::PluginsError>;
    fn set_balance(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError>;
    fn deposit(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError>;
    fn withdraw(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<Option<i64>, crate::error::PluginsError>;
}

/// JSON 文件存储（v6 默认；表结构与 v4 sqlite 同构）。
#[derive(Debug)]
struct FileEconomyStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileEconomyStore {
    fn new_default() -> Self {
        Self {
            path: economy_store_path(),
            lock: Mutex::new(()),
        }
    }

    fn load(&self) -> BTreeMap<String, i64> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => BTreeMap::new(),
        }
    }

    fn save(&self, balances: &BTreeMap<String, i64>) -> Result<(), crate::error::PluginsError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(balances)
            .map_err(|err| crate::error::PluginsError::EconomyStorage(err.to_string()))?;
        fs::write(&self.path, bytes)?;
        Ok(())
    }
}

impl EconomyStore for FileEconomyStore {
    fn balance(&self, player: &str, currency: &str) -> Result<i64, crate::error::PluginsError> {
        let _guard = self.lock.lock().expect("economy store lock poisoned");
        Ok(self.load().get(&balance_key(player, currency)).copied().unwrap_or(0))
    }

    fn set_balance(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        let _guard = self.lock.lock().expect("economy store lock poisoned");
        let mut balances = self.load();
        balances.insert(balance_key(player, currency), amount);
        self.save(&balances)?;
        Ok(amount)
    }

    fn deposit(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        let _guard = self.lock.lock().expect("economy store lock poisoned");
        let mut balances = self.load();
        let key = balance_key(player, currency);
        let next = balances.get(&key).copied().unwrap_or(0).saturating_add(amount);
        balances.insert(key, next);
        self.save(&balances)?;
        Ok(next)
    }

    fn withdraw(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<Option<i64>, crate::error::PluginsError> {
        let _guard = self.lock.lock().expect("economy store lock poisoned");
        let mut balances = self.load();
        let key = balance_key(player, currency);
        let current = balances.get(&key).copied().unwrap_or(0);
        if current < amount {
            return Ok(None);
        }
        let next = current - amount;
        balances.insert(key, next);
        self.save(&balances)?;
        Ok(Some(next))
    }
}

pub(crate) fn normalize_currency(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(crate) fn default_economy_storage() -> String {
    "sqlite".to_string()
}

fn economy_store_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config")
        .join("economy.json")
}

fn balance_key(player: &str, currency: &str) -> String {
    format!("{player}\t{currency}")
}

fn redis_key(player: &str, currency: &str) -> String {
    format!("qexed:economy:{currency}:{player}")
}

/// MySQL 经济存储（v4 MysqlEconomyStore 迁移；0.36 无 params! 宏，改 positional 参数）。
#[derive(Debug)]
struct MysqlEconomyStore {
    runtime: Mutex<tokio::runtime::Runtime>,
    opts: mysql_async::Opts,
}

impl MysqlEconomyStore {
    fn new(config: &crate::config::MysqlStorageConfig) -> Result<Self, crate::error::PluginsError> {
        config
            .validate()
            .map_err(crate::error::PluginsError::EconomyBackend)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| crate::error::PluginsError::EconomyBackend(format!("build mysql runtime: {err}")))?;
        let opts = mysql_async::Opts::from(
            mysql_async::OptsBuilder::default()
                .ip_or_hostname(config.ip.clone())
                .tcp_port(config.port)
                .user(Some(config.username.clone()))
                .pass(Some(config.password.clone()))
                .db_name(Some(config.database.clone())),
        );
        let store = Self {
            runtime: Mutex::new(runtime),
            opts,
        };
        store.block_on(async { store.ensure_table().await })?;
        Ok(store)
    }

    fn block_on<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, crate::error::PluginsError>>,
    ) -> Result<T, crate::error::PluginsError> {
        self.runtime
            .lock()
            .expect("mysql economy runtime poisoned")
            .block_on(future)
    }

    async fn conn(&self) -> Result<mysql_async::Conn, crate::error::PluginsError> {
        mysql_async::Conn::new(self.opts.clone())
            .await
            .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))
    }

    async fn ensure_table(&self) -> Result<(), crate::error::PluginsError> {
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
        .await
        .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
        Ok(())
    }
}

impl EconomyStore for MysqlEconomyStore {
    fn balance(&self, player: &str, currency: &str) -> Result<i64, crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let player = player.to_string();
        let currency = currency.to_string();
        self.block_on(async {
            let mut conn = self.conn().await?;
            Ok(conn
                .exec_first(
                    "SELECT `amount` FROM `qexed_economy_balances`
                     WHERE `player` = ? AND `currency` = ?",
                    (&player, &currency),
                )
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?
                .unwrap_or(0))
        })
    }

    fn set_balance(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let player = player.to_string();
        let currency = currency.to_string();
        self.block_on(async {
            let mut conn = self.conn().await?;
            conn.exec_drop(
                "INSERT INTO `qexed_economy_balances` (`player`, `currency`, `amount`)
                 VALUES (?, ?, ?)
                 ON DUPLICATE KEY UPDATE `amount` = VALUES(`amount`)",
                (&player, &currency, amount),
            )
            .await
            .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(amount)
        })
    }

    fn deposit(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let player = player.to_string();
        let currency = currency.to_string();
        self.block_on(async {
            let mut conn = self.conn().await?;
            conn.exec_drop(
                "INSERT INTO `qexed_economy_balances` (`player`, `currency`, `amount`)
                 VALUES (?, ?, ?)
                 ON DUPLICATE KEY UPDATE `amount` = `amount` + VALUES(`amount`)",
                (&player, &currency, amount),
            )
            .await
            .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(())
        })?;
        self.balance(&player, &currency)
    }

    fn withdraw(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<Option<i64>, crate::error::PluginsError> {
        use mysql_async::prelude::Queryable;

        let player = player.to_string();
        let currency = currency.to_string();
        let changed = self.block_on(async {
            let mut conn = self.conn().await?;
            conn.exec_drop(
                "UPDATE `qexed_economy_balances`
                 SET `amount` = `amount` - ?
                 WHERE `player` = ? AND `currency` = ? AND `amount` >= ?",
                (amount, &player, &currency, amount),
            )
            .await
            .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(conn.affected_rows())
        })?;
        if changed == 0 {
            return Ok(None);
        }
        self.balance(&player, &currency).map(Some)
    }
}

/// MongoDB 经济存储（v4 MongoEconomyStore 迁移）。
#[derive(Debug)]
struct MongoEconomyStore {
    runtime: Mutex<tokio::runtime::Runtime>,
    client: mongodb::Client,
    database: String,
}

impl MongoEconomyStore {
    fn new(config: &crate::config::MongoStorageConfig) -> Result<Self, crate::error::PluginsError> {
        config
            .validate()
            .map_err(crate::error::PluginsError::EconomyBackend)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| crate::error::PluginsError::EconomyBackend(format!("build mongo runtime: {err}")))?;
        let client = runtime
            .block_on(async {
                let mut options = mongodb::options::ClientOptions::parse(config.connection_uri())
                    .await
                    .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
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
                    .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))
            })?;
        // 懒连接驱动：new 里限时 ping 一次，不可达在 configure 阶段就暴露（槽位不装）。
        runtime
            .block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_millis(config.connect_timeout_ms),
                    client
                        .database(&config.database)
                        .run_command(mongodb::bson::doc! { "ping": 1 }),
                )
                .await
                .map_err(|_| crate::error::PluginsError::EconomyBackend("mongo ping timed out".to_string()))?
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
                Ok::<_, crate::error::PluginsError>(())
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
            .collection("qexed_economy_balances")
    }

    fn block_on<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, crate::error::PluginsError>>,
    ) -> Result<T, crate::error::PluginsError> {
        self.runtime
            .lock()
            .expect("mongo economy runtime poisoned")
            .block_on(future)
    }
}

impl EconomyStore for MongoEconomyStore {
    fn balance(&self, player: &str, currency: &str) -> Result<i64, crate::error::PluginsError> {
        let collection = self.collection();
        let key = balance_key(player, currency);
        self.block_on(async {
            let Some(doc) = collection
                .find_one(mongodb::bson::doc! { "_id": key })
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?
            else {
                return Ok(0);
            };
            Ok(doc.get_i64("amount").unwrap_or(0))
        })
    }

    fn set_balance(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        let collection = self.collection();
        let key = balance_key(player, currency);
        let player = player.to_string();
        let currency = currency.to_string();
        self.block_on(async {
            collection
                .update_one(
                    mongodb::bson::doc! { "_id": key },
                    mongodb::bson::doc! {
                        "$set": {
                            "player": player,
                            "currency": currency,
                            "amount": amount,
                            "updated_at": mongodb::bson::DateTime::now(),
                        }
                    },
                )
                .upsert(true)
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(amount)
        })
    }

    fn deposit(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        let collection = self.collection();
        let key = balance_key(player, currency);
        let player = player.to_string();
        let currency = currency.to_string();
        self.block_on(async {
            collection
                .update_one(
                    mongodb::bson::doc! { "_id": key },
                    mongodb::bson::doc! {
                        "$setOnInsert": { "player": player.clone(), "currency": currency.clone() },
                        "$inc": { "amount": amount },
                        "$set": { "updated_at": mongodb::bson::DateTime::now() },
                    },
                )
                .upsert(true)
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(())
        })?;
        self.balance(&player, &currency)
    }

    fn withdraw(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<Option<i64>, crate::error::PluginsError> {
        let collection = self.collection();
        let key = balance_key(player, currency);
        let withdrawn = self.block_on(async {
            let result = collection
                .update_one(
                    mongodb::bson::doc! {
                        "_id": key,
                        "amount": { "$gte": amount },
                    },
                    mongodb::bson::doc! {
                        "$inc": { "amount": -amount },
                        "$set": { "updated_at": mongodb::bson::DateTime::now() },
                    },
                )
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(result.modified_count > 0)
        })?;
        if !withdrawn {
            return Ok(None);
        }
        self.balance(player, currency).map(Some)
    }
}

/// Redis 经济存储（v4 RedisEconomyStore 迁移；withdraw 的 Lua 脚本保证原子性）。
#[derive(Debug)]
struct RedisEconomyStore {
    runtime: Mutex<tokio::runtime::Runtime>,
    client: redis::Client,
}

impl RedisEconomyStore {
    fn new(config: &crate::config::RedisStorageConfig) -> Result<Self, crate::error::PluginsError> {
        config
            .validate()
            .map_err(crate::error::PluginsError::EconomyBackend)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| crate::error::PluginsError::EconomyBackend(format!("build redis runtime: {err}")))?;
        let client = redis::Client::open(config.connection_url())
            .map_err(|err| crate::error::PluginsError::EconomyBackend(format!("redis url: {err}")))?;
        // 连接探活：限时 PING，不可达在 configure 阶段就暴露（槽位不装）。
        runtime
            .block_on(async {
                let mut conn = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    client.get_multiplexed_async_connection(),
                )
                .await
                .map_err(|_| crate::error::PluginsError::EconomyBackend("redis ping timed out".to_string()))?
                .map_err(|err| {
                    crate::error::PluginsError::EconomyBackend(format!("connect Redis economy store: {err}"))
                })?;
                let _: () = redis::cmd("PING")
                    .query_async(&mut conn)
                    .await
                    .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
                Ok::<_, crate::error::PluginsError>(())
            })?;
        Ok(Self {
            runtime: Mutex::new(runtime),
            client,
        })
    }

    fn block_on<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, crate::error::PluginsError>>,
    ) -> Result<T, crate::error::PluginsError> {
        self.runtime
            .lock()
            .expect("redis economy runtime poisoned")
            .block_on(future)
    }

    async fn connection(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, crate::error::PluginsError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| {
                crate::error::PluginsError::EconomyBackend(format!("connect Redis economy store: {err}"))
            })
    }
}

impl EconomyStore for RedisEconomyStore {
    fn balance(&self, player: &str, currency: &str) -> Result<i64, crate::error::PluginsError> {
        use redis::AsyncCommands;

        let key = redis_key(player, currency);
        self.block_on(async {
            let mut conn = self.connection().await?;
            Ok(conn.get(key).await.unwrap_or(0))
        })
    }

    fn set_balance(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        use redis::AsyncCommands;

        let key = redis_key(player, currency);
        self.block_on(async {
            let mut conn = self.connection().await?;
            let _: () = conn
                .set(key, amount)
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))?;
            Ok(amount)
        })
    }

    fn deposit(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<i64, crate::error::PluginsError> {
        use redis::AsyncCommands;

        let key = redis_key(player, currency);
        self.block_on(async {
            let mut conn = self.connection().await?;
            conn.incr(key, amount)
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))
        })
    }

    fn withdraw(
        &self,
        player: &str,
        currency: &str,
        amount: i64,
    ) -> Result<Option<i64>, crate::error::PluginsError> {
        let key = redis_key(player, currency);
        self.block_on(async {
            let mut conn = self.connection().await?;
            let script = redis::Script::new(
                "local value = tonumber(redis.call('GET', KEYS[1]) or '0')
                 local amount = tonumber(ARGV[1])
                 if value < amount then return false end
                 value = value - amount
                 redis.call('SET', KEYS[1], value)
                 return value",
            );
            script
                .key(key)
                .arg(amount)
                .invoke_async::<Option<i64>>(&mut conn)
                .await
                .map_err(|err| crate::error::PluginsError::EconomyBackend(err.to_string()))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: &str = "00000000-0000-0000-0000-000000000001";

    #[test]
    fn file_store_roundtrips_balances() {
        let dir = tempfile_dir();
        let store = FileEconomyStore {
            path: dir.join("economy.json"),
            lock: Mutex::new(()),
        };
        const CURRENCY: &str = "qexed:test";

        assert_eq!(store.balance(PLAYER, CURRENCY).unwrap(), 0);
        assert_eq!(store.set_balance(PLAYER, CURRENCY, 100).unwrap(), 100);
        assert_eq!(store.deposit(PLAYER, CURRENCY, 25).unwrap(), 125);
        assert_eq!(store.withdraw(PLAYER, CURRENCY, 200).unwrap(), None);
        assert_eq!(store.withdraw(PLAYER, CURRENCY, 40).unwrap(), Some(85));
        assert_eq!(store.balance(PLAYER, CURRENCY).unwrap(), 85);
    }

    #[test]
    fn state_routes_configured_currency_to_storage() {
        let mut state = EconomyState::default();
        // 端口 1 保证连接失败，避免本机恰有 MySQL 时测试抖动。
        let config = EconomyConfig {
            currencies: vec![crate::config::CurrencyConfig {
                id: "qexed:test".to_string(),
                storage: EconomyStorageEngine::Mysql,
                ..Default::default()
            }],
            mysql: crate::config::MysqlStorageConfig {
                port: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        state.configure(&config);

        assert_eq!(state.storage_for("qexed:test"), "mysql");
        assert!(state.set_balance("p", "qexed:test", 1).is_none());
    }

    /// 连接失败时槽位不装：engine 指向不可达端口，货币操作安全返回 None（不 panic）。
    #[test]
    fn mysql_store_not_installed_when_connect_fails() {
        let mut state = EconomyState::default();
        let config = EconomyConfig {
            currencies: vec![crate::config::CurrencyConfig {
                id: "qexed:fb".to_string(),
                storage: EconomyStorageEngine::Mysql,
                ..Default::default()
            }],
            mysql: crate::config::MysqlStorageConfig {
                port: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        state.configure(&config);

        assert_eq!(state.storage_for("qexed:fb"), "mysql");
        assert!(state.set_balance("p", "qexed:fb", 1).is_none());
    }

    #[test]
    fn redis_store_not_installed_when_connect_fails() {
        let mut state = EconomyState::default();
        let config = EconomyConfig {
            currencies: vec![crate::config::CurrencyConfig {
                id: "qexed:fr".to_string(),
                storage: EconomyStorageEngine::Redis,
                ..Default::default()
            }],
            redis: crate::config::RedisStorageConfig {
                port: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        state.configure(&config);

        assert!(state.balance("p", "qexed:fr").is_none());
    }

    /// 真实 MySQL 往返（需本地服务，默认跳过；v4 同名测试迁移）。
    #[test]
    #[ignore = "requires local MySQL on 127.0.0.1:3306"]
    fn mysql_store_roundtrips_balances() {
        let config = crate::config::MysqlStorageConfig::default();
        let store = MysqlEconomyStore::new(&config).expect("mysql economy store");
        let currency = "qexed:mysql_test";

        assert_eq!(store.set_balance(PLAYER, currency, 100).unwrap(), 100);
        assert_eq!(store.deposit(PLAYER, currency, 25).unwrap(), 125);
        assert_eq!(store.withdraw(PLAYER, currency, 200).unwrap(), None);
        assert_eq!(store.withdraw(PLAYER, currency, 40).unwrap(), Some(85));
    }

    /// 真实 MongoDB 往返（需本地服务，默认跳过；v4 同名测试迁移）。
    #[test]
    #[ignore = "requires local MongoDB on 127.0.0.1:27017"]
    fn mongodb_store_roundtrips_balances() {
        let config = crate::config::MongoStorageConfig {
            username: Some("qexed".to_string()),
            password: Some("qexed".to_string()),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let store = MongoEconomyStore::new(&config).expect("mongo economy store");
        let currency = "qexed:mongodb_test";

        assert_eq!(store.set_balance(PLAYER, currency, 100).unwrap(), 100);
        assert_eq!(store.deposit(PLAYER, currency, 25).unwrap(), 125);
        assert_eq!(store.withdraw(PLAYER, currency, 200).unwrap(), None);
        assert_eq!(store.withdraw(PLAYER, currency, 40).unwrap(), Some(85));
    }

    /// 真实 Redis 往返（需本地服务，默认跳过；v4 同名测试迁移）。
    #[test]
    #[ignore = "requires local Redis on 127.0.0.1:6379"]
    fn redis_store_roundtrips_balances() {
        let config = crate::config::RedisStorageConfig::default();
        let store = RedisEconomyStore::new(&config).expect("redis economy store");
        let currency = "qexed:redis_test";

        assert_eq!(store.set_balance(PLAYER, currency, 100).unwrap(), 100);
        assert_eq!(store.deposit(PLAYER, currency, 25).unwrap(), 125);
        assert_eq!(store.withdraw(PLAYER, currency, 200).unwrap(), None);
        assert_eq!(store.withdraw(PLAYER, currency, 40).unwrap(), Some(85));
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "qexed-plugins-eco-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}