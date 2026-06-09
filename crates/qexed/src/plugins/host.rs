use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
};

use wasmtime::Caller;

use super::PluginState;
use super::economy::{EconomyState, normalize_currency};
use super::structured_storage::StructuredStorageState;

const MAX_HOST_LOG_BYTES: usize = 16 * 1024;
const MAX_HOST_PATH_BYTES: usize = 1024;
const MAX_HOST_CONFIG_BYTES: usize = 256 * 1024;
const MAX_HOST_CURRENCY_BYTES: usize = 128;
const MAX_HOST_PATHFINDING_BYTES: usize = 16 * 1024;
const MAX_HOST_WORLD_EDIT_BYTES: usize = 8 * 1024;
const MAX_HOST_WORLD_EDIT_BATCH_BYTES: usize = 1024 * 1024;
const MAX_HOST_ENTITY_CONTROL_BYTES: usize = 16 * 1024;
const MAX_HOST_RANDOM_POOL_BYTES: usize = 256 * 1024;
const MAX_HOST_PLUGIN_API_BYTES: usize = 1024 * 1024;
const MAX_HOST_STORAGE_KEY_BYTES: usize = 512;
const MAX_HOST_STORAGE_VALUE_BYTES: usize = 1024 * 1024;
const MAX_RANDOM_POOL_PRECOMPUTE_COUNT: usize = 4096;
const RANDOM_POOL_REFILL_BATCH: usize = 16;
pub(super) const ECONOMY_ASYNC_PENDING: i64 = i64::MIN + 2;
pub(super) const STRUCTURED_STORAGE_ASYNC_PENDING: i64 = i64::MIN + 2;

#[derive(Debug, Default)]
pub(super) struct PluginHostServices {
    economy: Mutex<EconomyState>,
    economy_async: Mutex<EconomyAsyncState>,
    structured_storage: Mutex<StructuredStorageState>,
    structured_storage_async: Mutex<StructuredStorageAsyncState>,
    plugin_services: Mutex<PluginServiceState>,
    plugin_api: Mutex<Option<std::sync::Arc<dyn PluginApiService>>>,
    pathfinding: Mutex<Option<std::sync::Arc<dyn PathfindingService>>>,
    world_edit: Mutex<Option<std::sync::Arc<dyn WorldEditService>>>,
    entity_control: Mutex<Option<std::sync::Arc<dyn EntityControlService>>>,
    random_pools: Mutex<RandomPoolState>,
}

#[derive(Debug, Default)]
struct EconomyAsyncState {
    next_id: i64,
    requests: HashMap<i64, Receiver<i64>>,
}

#[derive(Debug, Default)]
struct StructuredStorageAsyncState {
    next_id: i64,
    requests: HashMap<i64, Receiver<Option<Vec<u8>>>>,
}

#[derive(Debug)]
enum StructuredStorageOperation {
    Exists,
    Get,
    Set(Vec<u8>),
    Delete,
}

#[derive(Debug)]
enum PollStructuredStorageResult {
    Pending,
    Ready(Vec<u8>),
    Failed,
}

pub(crate) trait PluginApiService: Send + Sync + std::fmt::Debug {
    fn call(&self, caller: &str, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>>;
}

#[derive(Debug, Default)]
struct PluginServiceState {
    available_plugins: HashSet<String>,
    providers: HashMap<String, String>,
}

pub(crate) trait PathfindingService: Send + Sync + std::fmt::Debug {
    fn find_path(&self, query: &str) -> Option<String>;
}

pub(crate) trait WorldEditService: Send + Sync + std::fmt::Debug {
    fn set_block(&self, query: &str) -> i32;
    fn set_blocks(&self, query: &str) -> i32;
    fn break_block(&self, query: &str) -> i32;
    fn register_region(&self, query: &str) -> i32;
}

pub(crate) trait EntityControlService: Send + Sync + std::fmt::Debug {
    fn upsert(&self, plugin_name: &str, query: &str) -> i32;
    fn move_entity(&self, plugin_name: &str, query: &str) -> i32;
    fn remove(&self, plugin_name: &str, query: &str) -> i32;
}

impl PluginHostServices {
    pub(super) fn configure_economy(&self, config: &qexed_config::app::qexed::server::Economy) {
        self.economy
            .lock()
            .expect("plugin economy state poisoned")
            .configure(config);
    }

    pub(super) fn configure_structured_storage(
        &self,
        config: &qexed_config::app::qexed::server::PluginStructuredStorage,
    ) {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .configure(config);
    }

    fn structured_storage_exists(&self, plugin: &str, key: &str) -> bool {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .exists(plugin, key)
    }

    fn structured_storage_get(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .get(plugin, key)
    }

    fn structured_storage_set(&self, plugin: &str, key: &str, value: &[u8]) -> bool {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .set(plugin, key, value)
    }

    fn structured_storage_delete(&self, plugin: &str, key: &str) -> bool {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .delete(plugin, key)
    }

    fn submit_structured_storage_async(
        self: &Arc<Self>,
        plugin: String,
        key: String,
        operation: StructuredStorageOperation,
    ) -> i64 {
        let (sender, receiver) = mpsc::channel();
        let id = {
            let mut state = self
                .structured_storage_async
                .lock()
                .expect("plugin structured storage async state poisoned");
            state.next_id = state.next_id.saturating_add(1).max(1);
            let id = state.next_id;
            state.requests.insert(id, receiver);
            id
        };
        let services = Arc::clone(self);
        std::thread::spawn(move || {
            let result = match operation {
                StructuredStorageOperation::Exists => Some(vec![u8::from(
                    services.structured_storage_exists(&plugin, &key),
                )]),
                StructuredStorageOperation::Get => services.structured_storage_get(&plugin, &key),
                StructuredStorageOperation::Set(value) => Some(vec![u8::from(
                    services.structured_storage_set(&plugin, &key, &value),
                )]),
                StructuredStorageOperation::Delete => Some(vec![u8::from(
                    services.structured_storage_delete(&plugin, &key),
                )]),
            };
            let _ = sender.send(result);
        });
        id
    }

    fn poll_structured_storage_async(&self, id: i64) -> PollStructuredStorageResult {
        if id <= 0 {
            return PollStructuredStorageResult::Failed;
        }
        let mut state = self
            .structured_storage_async
            .lock()
            .expect("plugin structured storage async state poisoned");
        let Some(receiver) = state.requests.get(&id) else {
            return PollStructuredStorageResult::Failed;
        };
        match receiver.try_recv() {
            Ok(value) => {
                state.requests.remove(&id);
                match value {
                    Some(value) => PollStructuredStorageResult::Ready(value),
                    None => PollStructuredStorageResult::Failed,
                }
            }
            Err(mpsc::TryRecvError::Empty) => PollStructuredStorageResult::Pending,
            Err(mpsc::TryRecvError::Disconnected) => {
                state.requests.remove(&id);
                PollStructuredStorageResult::Failed
            }
        }
    }

    fn forget_structured_storage_async(&self, id: i64) -> i32 {
        if id <= 0 {
            return -1;
        }
        self.structured_storage_async
            .lock()
            .expect("plugin structured storage async state poisoned")
            .requests
            .remove(&id)
            .map(|_| 0)
            .unwrap_or(-1)
    }

    pub(super) fn set_plugin_services(
        &self,
        plugins: impl IntoIterator<Item = String>,
        services: impl IntoIterator<Item = (String, String)>,
    ) {
        let mut state = self
            .plugin_services
            .lock()
            .expect("plugin service state poisoned");
        state.available_plugins = plugins.into_iter().collect();
        state.providers = services.into_iter().collect();
    }

    pub(super) fn set_plugin_api(&self, plugin_api: std::sync::Arc<dyn PluginApiService>) {
        *self.plugin_api.lock().expect("plugin API service poisoned") = Some(plugin_api);
    }

    fn has_plugin_service(&self, service: &str) -> bool {
        self.plugin_services
            .lock()
            .expect("plugin service state poisoned")
            .providers
            .contains_key(service.trim())
    }

    fn call_plugin_api(
        &self,
        caller: &str,
        service: &str,
        method: &str,
        payload: &[u8],
    ) -> Option<Vec<u8>> {
        self.plugin_api
            .lock()
            .expect("plugin API service poisoned")
            .as_ref()
            .and_then(|api| api.call(caller, service, method, payload))
    }

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

    fn set_blocks(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.set_blocks(query))
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

    pub(super) fn set_entity_control(
        &self,
        entity_control: std::sync::Arc<dyn EntityControlService>,
    ) {
        *self
            .entity_control
            .lock()
            .expect("entity control service poisoned") = Some(entity_control);
    }

    fn entity_upsert(&self, plugin_name: &str, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.upsert(plugin_name, query))
            .unwrap_or(-1)
    }

    fn entity_move(&self, plugin_name: &str, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.move_entity(plugin_name, query))
            .unwrap_or(-1)
    }

    fn entity_remove(&self, plugin_name: &str, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.remove(plugin_name, query))
            .unwrap_or(-1)
    }

    fn submit_economy_async(
        self: &Arc<Self>,
        player: String,
        currency: String,
        amount: i64,
        update: EconomyUpdate,
    ) -> i64 {
        let (sender, receiver) = mpsc::channel();
        let id = {
            let mut state = self
                .economy_async
                .lock()
                .expect("plugin economy async state poisoned");
            state.next_id = state.next_id.saturating_add(1).max(1);
            let id = state.next_id;
            state.requests.insert(id, receiver);
            id
        };
        let services = Arc::clone(self);
        std::thread::spawn(move || {
            let result = economy_update_inner(&services, &player, &currency, amount, update);
            let _ = sender.send(result);
        });
        id
    }

    fn poll_economy_async(&self, id: i64) -> i64 {
        if id <= 0 {
            return i64::MIN;
        }
        let mut state = self
            .economy_async
            .lock()
            .expect("plugin economy async state poisoned");
        let Some(receiver) = state.requests.get(&id) else {
            return i64::MIN;
        };
        match receiver.try_recv() {
            Ok(value) => {
                state.requests.remove(&id);
                value
            }
            Err(mpsc::TryRecvError::Empty) => ECONOMY_ASYNC_PENDING,
            Err(mpsc::TryRecvError::Disconnected) => {
                state.requests.remove(&id);
                i64::MIN
            }
        }
    }

    fn forget_economy_async(&self, id: i64) -> i32 {
        if id <= 0 {
            return -1;
        }
        self.economy_async
            .lock()
            .expect("plugin economy async state poisoned")
            .requests
            .remove(&id)
            .map(|_| 0)
            .unwrap_or(-1)
    }
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

pub(super) fn host_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

pub(super) fn host_plugin_service_exists(
    mut caller: Caller<'_, PluginState>,
    service_ptr: i32,
    service_len: i32,
) -> i32 {
    let Some(service) = host_string(&mut caller, service_ptr, service_len, 256) else {
        return -1;
    };
    i32::from(caller.data().services.has_plugin_service(&service))
}

pub(super) fn host_plugin_call(
    mut caller: Caller<'_, PluginState>,
    service_ptr: i32,
    service_len: i32,
    method_ptr: i32,
    method_len: i32,
    payload_ptr: i32,
    payload_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let caller_name = caller.data().name.clone();
    let Some(service) = host_string(&mut caller, service_ptr, service_len, 256) else {
        return -1;
    };
    let Some(method) = host_string(&mut caller, method_ptr, method_len, 256) else {
        return -1;
    };
    let Some(payload) = host_memory_bytes(
        &mut caller,
        payload_ptr,
        payload_len,
        MAX_HOST_PLUGIN_API_BYTES,
    )
    .map(|payload| payload.to_vec()) else {
        return -1;
    };
    let Some(response) =
        caller
            .data()
            .services
            .call_plugin_api(&caller_name, &service, &method, &payload)
    else {
        return -1;
    };
    write_host_response(&mut caller, out_ptr, out_len, &response)
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

pub(super) fn host_structured_storage_exists(
    mut caller: Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, ptr, len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    i32::from(
        caller
            .data()
            .services
            .structured_storage_exists(&plugin_name, &key),
    )
}

pub(super) fn host_structured_storage_get(
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
    let Some(bytes) = caller
        .data()
        .services
        .structured_storage_get(&plugin_name, &key)
    else {
        return -1;
    };
    if bytes.len() > MAX_HOST_STORAGE_VALUE_BYTES {
        return -1;
    }
    write_host_response(&mut caller, out_ptr, out_len, &bytes)
}

pub(super) fn host_structured_storage_set(
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
    let bytes = bytes.to_vec();
    if caller
        .data()
        .services
        .structured_storage_set(&plugin_name, &key, &bytes)
    {
        0
    } else {
        -1
    }
}

pub(super) fn host_structured_storage_delete(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
) -> i32 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    if caller
        .data()
        .services
        .structured_storage_delete(&plugin_name, &key)
    {
        0
    } else {
        -1
    }
}

pub(super) fn host_structured_storage_exists_async(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
) -> i64 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    caller.data().services.submit_structured_storage_async(
        plugin_name,
        key,
        StructuredStorageOperation::Exists,
    )
}

pub(super) fn host_structured_storage_get_async(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
) -> i64 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    caller.data().services.submit_structured_storage_async(
        plugin_name,
        key,
        StructuredStorageOperation::Get,
    )
}

pub(super) fn host_structured_storage_set_async(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i64 {
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
    let bytes = bytes.to_vec();
    caller.data().services.submit_structured_storage_async(
        plugin_name,
        key,
        StructuredStorageOperation::Set(bytes),
    )
}

pub(super) fn host_structured_storage_delete_async(
    mut caller: Caller<'_, PluginState>,
    key_ptr: i32,
    key_len: i32,
) -> i64 {
    let plugin_name = caller.data().name.clone();
    let Some(key) = host_string(&mut caller, key_ptr, key_len, MAX_HOST_STORAGE_KEY_BYTES) else {
        return -1;
    };
    caller.data().services.submit_structured_storage_async(
        plugin_name,
        key,
        StructuredStorageOperation::Delete,
    )
}

pub(super) fn host_structured_storage_async_poll(
    mut caller: Caller<'_, PluginState>,
    id: i64,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    match caller.data().services.poll_structured_storage_async(id) {
        PollStructuredStorageResult::Pending => STRUCTURED_STORAGE_ASYNC_PENDING,
        PollStructuredStorageResult::Ready(bytes) => {
            write_host_response(&mut caller, out_ptr, out_len, &bytes)
        }
        PollStructuredStorageResult::Failed => -1,
    }
}

pub(super) fn host_structured_storage_async_forget(
    caller: Caller<'_, PluginState>,
    id: i64,
) -> i32 {
    caller.data().services.forget_structured_storage_async(id)
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
    caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned")
        .register_currency(id, name, symbol, fractional_digits);
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
    let info = {
        let economy = caller
            .data()
            .services
            .economy
            .lock()
            .expect("plugin economy state poisoned");
        economy.currency_info(&currency)
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

pub(super) fn host_economy_storage(
    mut caller: Caller<'_, PluginState>,
    currency_ptr: i32,
    currency_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let Some(currency) = host_string(
        &mut caller,
        currency_ptr,
        currency_len,
        MAX_HOST_CURRENCY_BYTES,
    ) else {
        return -1;
    };
    let storage = caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned")
        .storage_for(&currency);
    write_host_response(&mut caller, out_ptr, out_len, storage.as_bytes())
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
    let economy = caller
        .data()
        .services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    let Some(currency) = economy.normalize_existing_currency(&currency) else {
        return i64::MIN;
    };
    economy.balance(&player, &currency).unwrap_or(i64::MIN)
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

pub(super) fn host_economy_balance_async(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
) -> i64 {
    economy_async_submit(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        0,
        EconomyUpdate::Balance,
    )
}

pub(super) fn host_economy_set_balance_async(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
) -> i64 {
    economy_async_submit(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        amount,
        EconomyUpdate::Set,
    )
}

pub(super) fn host_economy_deposit_async(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
) -> i64 {
    economy_async_submit(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        amount,
        EconomyUpdate::Deposit,
    )
}

pub(super) fn host_economy_withdraw_async(
    mut caller: Caller<'_, PluginState>,
    player_ptr: i32,
    player_len: i32,
    currency_ptr: i32,
    currency_len: i32,
    amount: i64,
) -> i64 {
    economy_async_submit(
        &mut caller,
        player_ptr,
        player_len,
        currency_ptr,
        currency_len,
        amount,
        EconomyUpdate::Withdraw,
    )
}

pub(super) fn host_economy_async_poll(caller: Caller<'_, PluginState>, id: i64) -> i64 {
    caller.data().services.poll_economy_async(id)
}

pub(super) fn host_economy_async_forget(caller: Caller<'_, PluginState>, id: i64) -> i32 {
    caller.data().services.forget_economy_async(id)
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

pub(super) fn host_world_set_blocks(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(
        &mut caller,
        query_ptr,
        query_len,
        MAX_HOST_WORLD_EDIT_BATCH_BYTES,
    ) else {
        return -1;
    };
    caller.data().services.set_blocks(&query)
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

pub(super) fn host_entity_upsert(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(
        &mut caller,
        query_ptr,
        query_len,
        MAX_HOST_ENTITY_CONTROL_BYTES,
    ) else {
        return -1;
    };
    let plugin_name = caller.data().name.clone();
    caller.data().services.entity_upsert(&plugin_name, &query)
}

pub(super) fn host_entity_move(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(
        &mut caller,
        query_ptr,
        query_len,
        MAX_HOST_ENTITY_CONTROL_BYTES,
    ) else {
        return -1;
    };
    let plugin_name = caller.data().name.clone();
    caller.data().services.entity_move(&plugin_name, &query)
}

pub(super) fn host_entity_remove(
    mut caller: Caller<'_, PluginState>,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = host_string(
        &mut caller,
        query_ptr,
        query_len,
        MAX_HOST_ENTITY_CONTROL_BYTES,
    ) else {
        return -1;
    };
    let plugin_name = caller.data().name.clone();
    caller.data().services.entity_remove(&plugin_name, &query)
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

#[derive(Debug, Clone, Copy)]
enum EconomyUpdate {
    Balance,
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
    economy_update_inner(&caller.data().services, &player, &currency, amount, update)
}

fn economy_async_submit(
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
    caller
        .data()
        .services
        .submit_economy_async(player, currency, amount, update)
}

fn economy_update_inner(
    services: &Arc<PluginHostServices>,
    player: &str,
    currency: &str,
    amount: i64,
    update: EconomyUpdate,
) -> i64 {
    let economy = services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    let Some(currency) = economy.normalize_existing_currency(&currency) else {
        return i64::MIN;
    };
    let result = match update {
        EconomyUpdate::Balance => economy.balance(player, &currency),
        EconomyUpdate::Set => economy.set_balance(&player, &currency, amount),
        EconomyUpdate::Deposit => economy.deposit(&player, &currency, amount),
        EconomyUpdate::Withdraw => match economy.withdraw(&player, &currency, amount) {
            Some(balance) => Some(balance),
            None => return i64::MIN + 1,
        },
    };
    result.unwrap_or(i64::MIN)
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
