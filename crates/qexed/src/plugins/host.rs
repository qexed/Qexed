use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use wasmtime::Caller;

use super::PluginState;

const MAX_HOST_LOG_BYTES: usize = 16 * 1024;
const MAX_HOST_PATH_BYTES: usize = 1024;
const MAX_HOST_CONFIG_BYTES: usize = 256 * 1024;
const DEFAULT_CURRENCY: &str = "qexed:coin";

#[derive(Debug, Default)]
pub(super) struct PluginHostServices {
    economy: Mutex<EconomyState>,
}

#[derive(Debug)]
struct EconomyState {
    currencies: BTreeMap<String, CurrencyInfo>,
    balances: BTreeMap<(String, String), i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CurrencyInfo {
    id: String,
    name: String,
    symbol: String,
    fractional_digits: i32,
}

impl Default for EconomyState {
    fn default() -> Self {
        Self::load().unwrap_or_else(Self::with_default_currency)
    }
}

impl EconomyState {
    fn with_default_currency() -> Self {
        let mut currencies = BTreeMap::new();
        currencies.insert(
            DEFAULT_CURRENCY.to_string(),
            CurrencyInfo {
                id: DEFAULT_CURRENCY.to_string(),
                name: "Coin".to_string(),
                symbol: "Q".to_string(),
                fractional_digits: 2,
            },
        );
        Self {
            currencies,
            balances: BTreeMap::new(),
        }
    }

    fn load() -> Option<Self> {
        let path = economy_storage_path()?;
        let contents = fs::read_to_string(path).ok()?;
        let storage: EconomyStorage = toml::from_str(&contents).ok()?;
        let mut state = Self {
            currencies: storage
                .currencies
                .into_iter()
                .map(|currency| (normalize_currency(&currency.id), currency))
                .collect(),
            balances: storage
                .balances
                .into_iter()
                .map(|balance| {
                    (
                        (balance.player, normalize_currency(&balance.currency)),
                        balance.amount,
                    )
                })
                .collect(),
        };
        state.ensure_default_currency();
        Some(state)
    }

    fn save(&self) {
        let Some(path) = economy_storage_path() else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        let storage = EconomyStorage {
            currencies: self.currencies.values().cloned().collect(),
            balances: self
                .balances
                .iter()
                .map(|((player, currency), amount)| EconomyBalanceEntry {
                    player: player.clone(),
                    currency: currency.clone(),
                    amount: *amount,
                })
                .collect(),
        };
        let Ok(contents) = toml::to_string_pretty(&storage) else {
            log::warn!("plugin economy storage encode failed");
            return;
        };
        if let Err(err) = fs::create_dir_all(parent).and_then(|_| fs::write(path, contents)) {
            log::warn!("plugin economy storage write failed: {err}");
        }
    }

    fn ensure_default_currency(&mut self) {
        self.currencies
            .entry(DEFAULT_CURRENCY.to_string())
            .or_insert_with(|| CurrencyInfo {
                id: DEFAULT_CURRENCY.to_string(),
                name: "Coin".to_string(),
                symbol: "Q".to_string(),
                fractional_digits: 2,
            });
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct EconomyStorage {
    #[serde(default)]
    currencies: Vec<CurrencyInfo>,
    #[serde(default)]
    balances: Vec<EconomyBalanceEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EconomyBalanceEntry {
    player: String,
    currency: String,
    amount: i64,
}

pub(super) fn host_log(mut caller: Caller<'_, PluginState>, ptr: i32, len: i32) {
    let plugin_name = caller.data().name.clone();
    let Some(bytes) = host_memory_bytes(&mut caller, ptr, len, MAX_HOST_LOG_BYTES) else {
        return;
    };
    match std::str::from_utf8(bytes) {
        Ok(message) => log::info!("[WASM plugin:{plugin_name}] {message}"),
        Err(err) => log::warn!("[WASM plugin:{plugin_name}] log message is not UTF-8: {err}"),
    }
}

pub(super) fn host_config_exists(mut caller: Caller<'_, PluginState>, ptr: i32, len: i32) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(path) = host_string(&mut caller, ptr, len, MAX_HOST_PATH_BYTES) else {
        return -1;
    };
    match plugin_config_path(&plugin_name, &path) {
        Some(path) => i32::from(path.is_file()),
        None => -1,
    }
}

pub(super) fn host_config_read(
    mut caller: Caller<'_, PluginState>,
    path_ptr: i32,
    path_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let plugin_name = caller.data().name.clone();
    let Some(path) = host_string(&mut caller, path_ptr, path_len, MAX_HOST_PATH_BYTES) else {
        return -1;
    };
    let Some(path) = plugin_config_path(&plugin_name, &path) else {
        return -1;
    };
    let Ok(bytes) = fs::read(path) else {
        return -1;
    };
    if bytes.len() > MAX_HOST_CONFIG_BYTES {
        return -1;
    }
    write_host_response(&mut caller, out_ptr, out_len, &bytes)
}

pub(super) fn host_config_write(
    mut caller: Caller<'_, PluginState>,
    path_ptr: i32,
    path_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(path) = host_string(&mut caller, path_ptr, path_len, MAX_HOST_PATH_BYTES) else {
        return -1;
    };
    let Some(bytes) = host_memory_bytes(&mut caller, data_ptr, data_len, MAX_HOST_CONFIG_BYTES)
    else {
        return -1;
    };
    let Some(path) = plugin_config_path(&plugin_name, &path) else {
        return -1;
    };
    let Some(parent) = path.parent() else {
        return -1;
    };
    if fs::create_dir_all(parent).is_err() || fs::write(path, bytes).is_err() {
        return -1;
    }
    0
}

pub(super) fn host_economy_register_currency(
    mut caller: Caller<'_, PluginState>,
    id_ptr: i32,
    id_len: i32,
    name_ptr: i32,
    name_len: i32,
    symbol_ptr: i32,
    symbol_len: i32,
    fractional_digits: i32,
) -> i32 {
    let Some(id) = host_string(&mut caller, id_ptr, id_len, 128) else {
        return -1;
    };
    let Some(name) = host_string(&mut caller, name_ptr, name_len, 128) else {
        return -1;
    };
    let Some(symbol) = host_string(&mut caller, symbol_ptr, symbol_len, 32) else {
        return -1;
    };
    let id = normalize_currency(&id);
    if id.is_empty() {
        return -1;
    }
    let mut economy = caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    economy.currencies.insert(
        id.clone(),
        CurrencyInfo {
            id,
            name,
            symbol,
            fractional_digits: fractional_digits.clamp(0, 8),
        },
    );
    economy.save();
    0
}

pub(super) fn host_economy_currency_info(
    mut caller: Caller<'_, PluginState>,
    currency_ptr: i32,
    currency_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let Some(currency) = host_string(&mut caller, currency_ptr, currency_len, 128) else {
        return -1;
    };
    let currency = normalize_currency(&currency);
    let info = {
        let economy = caller
            .data()
            .services
            .economy
            .lock()
            .expect("plugin economy state poisoned");
        economy
            .currencies
            .get(if currency.is_empty() {
                DEFAULT_CURRENCY
            } else {
                &currency
            })
            .cloned()
    };
    let Some(info) = info else {
        return -1;
    };
    let encoded = format!(
        "{}\t{}\t{}\t{}",
        info.id, info.name, info.symbol, info.fractional_digits
    );
    write_host_response(&mut caller, out_ptr, out_len, encoded.as_bytes())
}

pub(super) fn host_economy_balance(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
) -> i64 {
    let Some(player) = host_string(&mut caller, player_ptr, player_len, 128) else {
        return i64::MIN;
    };
    let Some(currency) = host_string(&mut caller, currency_ptr, currency_len, 128) else {
        return i64::MIN;
    };
    let player = player.trim().to_string();
    if player.is_empty() {
        return i64::MIN;
    }
    let currency = normalized_existing_currency(&caller, &currency);
    let economy = caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    *economy.balances.get(&(player, currency)).unwrap_or(&0)
}

pub(super) fn host_economy_set_balance(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
) -> i64 {
    economy_update(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        amount,
        EconomyUpdate::Set,
    )
}

pub(super) fn host_economy_deposit(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
) -> i64 {
    economy_update(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        amount,
        EconomyUpdate::Deposit,
    )
}

pub(super) fn host_economy_withdraw(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
) -> i64 {
    economy_update(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        amount,
        EconomyUpdate::Withdraw,
    )
}

fn host_memory_bytes<'a>(
    caller: &'a mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
    max_len: usize,
) -> Option<&'a [u8]> {
    if ptr < 0 || len < 0 {
        log::warn!("WASM plugin passed a negative memory range: ptr={ptr}, len={len}");
        return None;
    }

    let offset = ptr as usize;
    let len = len as usize;
    if len > max_len {
        log::warn!("WASM plugin host API payload too large: len={len}, max={max_len}");
        return None;
    }

    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        log::warn!("WASM plugin host API call is missing memory export");
        return None;
    };
    let data = memory.data(&*caller);
    let end = offset.checked_add(len)?;
    if end > data.len() {
        log::warn!("WASM plugin host API memory out of bounds: ptr={ptr}, len={len}");
        return None;
    }
    Some(&data[offset..end])
}

fn host_memory_write(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
    bytes: &[u8],
) -> bool {
    if ptr < 0 || len < 0 || bytes.len() > len as usize {
        return false;
    }
    let offset = ptr as usize;
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        return false;
    };
    let data = memory.data_mut(caller);
    let Some(end) = offset.checked_add(bytes.len()) else {
        return false;
    };
    if end > data.len() {
        return false;
    }
    data[offset..end].copy_from_slice(bytes);
    true
}

fn host_string(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
    max_len: usize,
) -> Option<String> {
    let bytes = host_memory_bytes(caller, ptr, len, max_len)?;
    std::str::from_utf8(bytes).ok().map(str::to_string)
}

fn write_host_response(
    caller: &mut Caller<'_, PluginState>,
    out_ptr: i32,
    out_len: i32,
    bytes: &[u8],
) -> i64 {
    let out_len = out_len.max(0) as usize;
    if out_len < bytes.len() {
        return -i64::try_from(bytes.len()).unwrap_or(i64::MAX) - 1;
    }
    if !host_memory_write(caller, out_ptr, out_len as i32, bytes) {
        return -1;
    }
    i64::try_from(bytes.len()).unwrap_or(-1)
}

fn plugin_config_path(plugin_name: &str, requested: &str) -> Option<PathBuf> {
    let requested = requested.trim();
    let relative = if requested.is_empty() {
        Path::new("config.toml")
    } else {
        Path::new(requested)
    };
    if relative.is_absolute() {
        return None;
    }
    let mut clean = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => clean.push(value),
            _ => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        clean.push("config.toml");
    }
    Some(
        std::env::current_dir()
            .ok()?
            .join("config")
            .join("plugins")
            .join(plugin_name)
            .join(clean),
    )
}

fn economy_storage_path() -> Option<PathBuf> {
    Some(
        std::env::current_dir()
            .ok()?
            .join("config")
            .join("economy.toml"),
    )
}

fn normalize_currency(currency: &str) -> String {
    let currency = currency.trim();
    if currency.is_empty() {
        DEFAULT_CURRENCY.to_string()
    } else {
        currency.to_ascii_lowercase()
    }
}

fn normalized_existing_currency(caller: &Caller<'_, PluginState>, currency: &str) -> String {
    let currency = normalize_currency(currency);
    let economy = caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    if economy.currencies.contains_key(&currency) {
        currency
    } else {
        DEFAULT_CURRENCY.to_string()
    }
}

enum EconomyUpdate {
    Set,
    Deposit,
    Withdraw,
}

fn economy_update(
    caller: &mut Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
    update: EconomyUpdate,
) -> i64 {
    let Some(player) = host_string(caller, player_ptr, player_len, 128) else {
        return i64::MIN;
    };
    let Some(currency) = host_string(caller, currency_ptr, currency_len, 128) else {
        return i64::MIN;
    };
    let player = player.trim().to_string();
    if player.is_empty() || amount < 0 {
        return i64::MIN;
    }
    let currency = normalized_existing_currency(caller, &currency);
    let mut economy = caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    let new_balance = {
        let balance = economy.balances.entry((player, currency)).or_insert(0);
        match update {
            EconomyUpdate::Set => *balance = amount,
            EconomyUpdate::Deposit => *balance = balance.saturating_add(amount),
            EconomyUpdate::Withdraw => {
                if *balance < amount {
                    return i64::MIN + 1;
                }
                *balance -= amount;
            }
        }
        *balance
    };
    economy.save();
    new_balance
}
