//! 插件经济系统（v4 plugins/economy.rs 迁移）。
//!
//! v4 支持 sqlite/mysql/mongodb/redis 四种存储；v6 workspace 无
//! mysql/mongodb/redis 依赖（TODO(storage)），保留 EconomyStore trait 供
//! 后续接入，默认实现为 JSON 文件存储（config/economy.json，格式与 v4
//! sqlite 表同构：player+currency -> amount）。

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
        // sqlite 槽位用文件实现；其余引擎 TODO(storage)：无依赖，回退告警。
        self.configure_store("sqlite", || Ok(Arc::new(FileEconomyStore::new_default())));
        for engine in [EconomyStorageEngine::Mysql, EconomyStorageEngine::Mongodb, EconomyStorageEngine::Redis] {
            if self.uses_storage(engine) {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.economy.storage.unavailable")
                        .replace("%{engine}", engine.as_str())
                );
            }
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
        let store = self.store_for_currency(currency)?;
        store.set_balance(player, currency, amount).ok()
    }

    pub(crate) fn deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
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

/// 经济余额存储接口。
///
/// v4 为 async_trait + rusqlite/mysql/mongodb/redis 实现；v6 无这些依赖
/// （TODO(storage)），trait 保留同步签名，外部存储后端接入时实现此接口
/// 并通过 EconomyState::install 槽位扩展。
// TODO(storage): mysql/mongodb/redis 后端接入时提供实现
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_roundtrips_balances() {
        let dir = tempfile_dir();
        let store = FileEconomyStore {
            path: dir.join("economy.json"),
            lock: Mutex::new(()),
        };
        const PLAYER: &str = "00000000-0000-0000-0000-000000000001";
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
        let config = EconomyConfig {
            currencies: vec![crate::config::CurrencyConfig {
                id: "qexed:test".to_string(),
                storage: EconomyStorageEngine::Mysql,
                ..Default::default()
            }],
        };

        state.configure(&config);

        assert_eq!(state.storage_for("qexed:test"), "mysql");
        assert!(state.set_balance("p", "qexed:test", 1).is_none());
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
