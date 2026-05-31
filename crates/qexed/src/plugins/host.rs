use std::{
    collections::{BTreeMap, HashMap, VecDeque},
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
const MAX_HOST_PATHFINDING_BYTES: usize = 16 * 1024;
const MAX_HOST_WORLD_EDIT_BYTES: usize = 8 * 1024;
const MAX_HOST_RANDOM_POOL_BYTES: usize = 256 * 1024;
const MAX_HOST_STORAGE_KEY_BYTES: usize = 512;
const MAX_HOST_STORAGE_VALUE_BYTES: usize = 1024 * 1024;
const MAX_RANDOM_POOL_PRECOMPUTE_COUNT: usize = 4096;
const RANDOM_POOL_REFILL_BATCH: usize = 16;
const DEFAULT_CURRENCY: &str = "qexed:coin";

#[derive(Debug, Default)]
pub(super) struct PluginHostServices {
    economy: Mutex<EconomyState>,
    pathfinding: Mutex<Option<std::sync::Arc<dyn PathfindingService>>>,
    world_edit: Mutex<Option<std::sync::Arc<dyn WorldEditService>>>,
    random_pools: Mutex<RandomPoolState>,
}

pub(crate) trait PathfindingService: Send + Sync + std::fmt::Debug {
    fn find_path(&self, query: &str) -> Option<String>;
}

pub(crate) trait WorldEditService: Send + Sync + std::fmt::Debug {
    fn set_block(&self, query: &str) -> i32;
    fn break_block(&self, query: &str) -> i32;
    fn register_region(&self, query: &str) -> i32;
}

impl PluginHostServices {
    pub(super) fn set_pathfinding(&self, pathfinding: std::sync::Arc<dyn PathfindingService>) {
        *self
            .pathfinding
            .lock()
            .expect("pathfinding service poisoned") = Some(pathfinding);
    }

    fn find_path(&self, query: &str) -> Option<String> {
        self.pathfinding
            .lock()
            .expect("pathfinding service poisoned")
            .as_ref()
            .and_then(|service| service.find_path(query))
    }

    pub(super) fn set_world_edit(&self, world_edit: std::sync::Arc<dyn WorldEditService>) {
        *self.world_edit.lock().expect("world edit service poisoned") = Some(world_edit);
    }

    fn set_block(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.set_block(query))
            .unwrap_or(-1)
    }

    fn break_block(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.break_block(query))
            .unwrap_or(-1)
    }

    fn register_region(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.register_region(query))
            .unwrap_or(-1)
    }
}

#[derive(Debug)]
struct EconomyState {
    currencies: BTreeMap<String, CurrencyInfo>,
    balances: BTreeMap<(String, String), i64>,
}

#[derive(Debug, Default)]
struct RandomPoolState {
    pools: HashMap<String, RandomPool>,
}

#[derive(Debug, Clone)]
struct RandomPool {
    precompute_count: usize,
    entries: Vec<RandomPoolEntry>,
    cached: VecDeque<String>,
    signature: String,
}

#[derive(Debug, Clone)]
struct RandomPoolEntry {
    weight: u64,
    value: String,
}

impl RandomPoolState {
    fn roll(&mut self, request: &str) -> Option<String> {
        let request = RandomPoolRequest::parse(request)?;
        let signature = request.signature();
        let pool = self
            .pools
            .entry(request.key())
            .and_modify(|pool| {
                if pool.signature != signature {
                    *pool = RandomPool::new(&request, &signature);
                }
            })
            .or_insert_with(|| RandomPool::new(&request, &signature));
        pool.roll()
    }
}

impl RandomPool {
    fn new(request: &RandomPoolRequest, signature: &str) -> Self {
        Self {
            precompute_count: request
                .precompute_count
                .min(MAX_RANDOM_POOL_PRECOMPUTE_COUNT),
            entries: request.entries.clone(),
            cached: VecDeque::new(),
            signature: signature.to_string(),
        }
    }

    fn roll(&mut self) -> Option<String> {
        if self.precompute_count == 0 {
            return weighted_pick(&self.entries);
        }
        let refilled_empty_cache = self.cached.is_empty();
        if refilled_empty_cache {
            self.refill();
        }
        let value = self.cached.pop_front();
        if !refilled_empty_cache && self.cached.len() < self.precompute_count / 2 {
            self.refill();
        }
        value
    }

    fn refill(&mut self) {
        let missing = self.precompute_count.saturating_sub(self.cached.len());
        let count = missing.min(RANDOM_POOL_REFILL_BATCH);
        for _ in 0..count {
            let Some(value) = weighted_pick(&self.entries) else {
                break;
            };
            self.cached.push_back(value);
        }
    }
}

#[derive(Debug)]
struct RandomPoolRequest {
    id: String,
    kind: String,
    precompute_count: usize,
    entries: Vec<RandomPoolEntry>,
}

impl RandomPoolRequest {
    fn parse(request: &str) -> Option<Self> {
        let mut lines = request.lines();
        let header = lines.next()?.trim();
        let mut parts = header.split('\t');
        let id = parts.next()?.trim().to_string();
        let kind = parts.next()?.trim().to_string();
        let precompute_count = parts.next()?.trim().parse::<usize>().ok()?;
        if id.is_empty() || kind.is_empty() {
            return None;
        }

        let mut entries = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((weight, value)) = line.split_once('\t') else {
                continue;
            };
            let Ok(weight) = weight.trim().parse::<u64>() else {
                continue;
            };
            let value = value.trim();
            if weight == 0 || value.is_empty() {
                continue;
            }
            entries.push(RandomPoolEntry {
                weight,
                value: value.to_string(),
            });
        }
        (!entries.is_empty()).then_some(Self {
            id,
            kind,
            precompute_count,
            entries,
        })
    }

    fn key(&self) -> String {
        format!("{}\t{}", self.kind, self.id)
    }

    fn signature(&self) -> String {
        let mut signature = format!("{}\t{}\t{}", self.id, self.kind, self.precompute_count);
        for entry in &self.entries {
            signature.push('\n');
            signature.push_str(&entry.weight.to_string());
            signature.push('\t');
            signature.push_str(&entry.value);
        }
        signature
    }
}

fn weighted_pick(entries: &[RandomPoolEntry]) -> Option<String> {
    let total = entries
        .iter()
        .fold(0u64, |total, entry| total.saturating_add(entry.weight));
    if total == 0 {
        return None;
    }

    let mut pick = rand::Rng::gen_range(&mut rand::thread_rng(), 0..total);
    for entry in entries {
        if pick < entry.weight {
            return Some(entry.value.clone());
        }
        pick -= entry.weight;
    }
    None
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

pub(super) fn host_storage_exists(mut caller: Caller<'_, PluginState>, ptr: i32, len: i32) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, ptr, len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    match plugin_storage_path(&plugin_name, &key) {
        Some(path) => i32::from(path.is_file()),
        None => -1,
    }
}

pub(super) fn host_storage_get(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    let Some(path) = plugin_storage_path(&plugin_name, &key) else {
        return -1;
    };
    let Ok(bytes) = fs::read(path) else {
        return -1;
    };
    if bytes.len() > MAX_HOST_STORAGE_VALUE_BYTES {
        return -1;
    }
    write_host_response(&mut caller, out_ptr, out_len, &bytes)
}

pub(super) fn host_storage_set(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    let Some(bytes) = host_memory_bytes(
        &mut caller,
        data_ptr,
        data_len,
        MAX_HOST_STORAGE_VALUE_BYTES,
    ) else {
        return -1;
    };
    let Some(path) = plugin_storage_path(&plugin_name, &key) else {
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

pub(super) fn host_storage_delete(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    let Some(path) = plugin_storage_path(&plugin_name, &key) else {
        return -1;
    };
    match fs::remove_file(path) {
        Ok(()) => 0,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => -1,
    }
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

pub(super) fn host_lottery_roll(
    mut caller: Caller<'_, PluginState>,
    entries_ptr: i32,
    entries_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let Some(entries) = host_string(&mut caller, entries_ptr, entries_len, MAX_HOST_CONFIG_BYTES)
    else {
        return -1;
    };
    let mut total = 0u64;
    let mut parsed = Vec::<(u64, String)>::new();
    for line in entries.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((weight, value)) = line.split_once('\t') else {
            continue;
        };
        let Ok(weight) = weight.trim().parse::<u64>() else {
            continue;
        };
        if weight == 0 {
            continue;
        }
        total = total.saturating_add(weight);
        parsed.push((weight, value.trim().to_string()));
    }
    if total == 0 || parsed.is_empty() {
        return -1;
    }

    let mut pick = rand::Rng::gen_range(&mut rand::thread_rng(), 0..total);
    for (weight, value) in parsed {
        if pick < weight {
            return write_host_response(&mut caller, out_ptr, out_len, value.as_bytes());
        }
        pick -= weight;
    }
    -1
}

pub(super) fn host_pathfinding_find(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let Some(query) = host_string(
        &mut caller,
        query_ptr,
        query_len,
        MAX_HOST_PATHFINDING_BYTES,
    ) else {
        return -1;
    };
    let Some(response) = caller.data().services.find_path(&query) else {
        return -1;
    };
    write_host_response(&mut caller, out_ptr, out_len, response.as_bytes())
}

pub(super) fn host_world_set_block(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(&mut caller, query_ptr, query_len, MAX_HOST_WORLD_EDIT_BYTES)
    else {
        return -1;
    };
    caller.data().services.set_block(&query)
}

pub(super) fn host_world_break_block(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(&mut caller, query_ptr, query_len, MAX_HOST_WORLD_EDIT_BYTES)
    else {
        return -1;
    };
    caller.data().services.break_block(&query)
}

pub(super) fn host_world_register_edit_region(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(&mut caller, query_ptr, query_len, MAX_HOST_WORLD_EDIT_BYTES)
    else {
        return -1;
    };
    caller.data().services.register_region(&query)
}

pub(super) fn host_random_pool_roll(
    mut caller: Caller<'_, PluginState>,
    request_ptr: i32,
    request_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let Some(request) = host_string(
        &mut caller,
        request_ptr,
        request_len,
        MAX_HOST_RANDOM_POOL_BYTES,
    ) else {
        return -1;
    };
    let value = caller
        .data()
        .services
        .random_pools
        .lock()
        .expect("plugin random pool state poisoned")
        .roll(&request);
    let Some(value) = value else {
        return -1;
    };
    write_host_response(&mut caller, out_ptr, out_len, value.as_bytes())
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

fn plugin_storage_path(plugin_name: &str, key: &str) -> Option<PathBuf> {
    let clean = clean_storage_key(key)?;
    Some(
        std::env::current_dir()
            .ok()?
            .join("config")
            .join("plugins")
            .join(plugin_name)
            .join("storage")
            .join(clean),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_pool_rolls_realtime_entries() {
        let mut state = RandomPoolState::default();
        let value = state
            .roll("ore\tblock\t0\n1\tminecraft:stone\n")
            .expect("random pool value");

        assert_eq!(value, "minecraft:stone");
    }

    #[test]
    fn random_pool_refills_precomputed_entries_lazily() {
        let mut state = RandomPoolState::default();
        let request = "ore\tblock\t32\n1\tminecraft:stone\n";

        assert_eq!(state.roll(request).as_deref(), Some("minecraft:stone"));
        let pool = state
            .pools
            .get("block\tore")
            .expect("pool should be retained");

        assert_eq!(pool.precompute_count, 32);
        assert!(pool.cached.len() < RANDOM_POOL_REFILL_BATCH);
    }

    #[test]
    fn random_pool_replaces_cache_when_entries_change() {
        let mut state = RandomPoolState::default();

        assert_eq!(
            state.roll("ore\tblock\t4\n1\tminecraft:stone\n").as_deref(),
            Some("minecraft:stone")
        );
        assert_eq!(
            state.roll("ore\tblock\t4\n1\tminecraft:dirt\n").as_deref(),
            Some("minecraft:dirt")
        );
    }
}
