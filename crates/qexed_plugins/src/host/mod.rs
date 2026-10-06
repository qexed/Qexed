//! 插件宿主服务（v4 plugins/host.rs 迁移；v6 为 libloading dll/so 插件提供宿主能力）。
//!
//! v4 通过 wasmtime Caller + WASM 线性内存交互；v6 动态库插件与本进程共享
//! 地址空间，宿主能力以 HostContext（vtable + 插件名）形式暴露给插件，
//! 所有 host_* 入口保持与 v4 同名的语义与字节上限。

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
};

use crate::api::{GeyserPlayerInfoQuery, GeyserPlayerInfoResponse, HttpResponse};
use crate::economy::{EconomyState, normalize_currency};
use crate::structured_storage::StructuredStorageState;

pub(crate) const MAX_HOST_LOG_BYTES: usize = 16 * 1024;
pub(crate) const MAX_HOST_PATH_BYTES: usize = 1024;
pub(crate) const MAX_HOST_CONFIG_BYTES: usize = 256 * 1024;
pub(crate) const MAX_HOST_CURRENCY_BYTES: usize = 128;
pub(crate) const MAX_HOST_PATHFINDING_BYTES: usize = 16 * 1024;
pub(crate) const MAX_HOST_WORLD_EDIT_BYTES: usize = 8 * 1024;
pub(crate) const MAX_HOST_WORLD_EDIT_BATCH_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_HOST_ENTITY_CONTROL_BYTES: usize = 16 * 1024;
pub(crate) const MAX_HOST_RANDOM_POOL_BYTES: usize = 256 * 1024;
pub(crate) const MAX_HOST_PLUGIN_API_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_HOST_STORAGE_KEY_BYTES: usize = 512;
pub(crate) const MAX_HOST_STORAGE_VALUE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_RANDOM_POOL_PRECOMPUTE_COUNT: usize = 4096;
pub(crate) const RANDOM_POOL_REFILL_BATCH: usize = 16;
pub(super) const ECONOMY_ASYNC_PENDING: i64 = i64::MIN + 2;
pub(super) const STRUCTURED_STORAGE_ASYNC_PENDING: i64 = i64::MIN + 2;

/// 插件可用的宿主能力表（动态库插件通过 qexed_plugin_host_table 导出拿到指针）。
///
/// v4 在 WASM Linker 上注册约 50 个 host 函数；v6 合并为单一 call 入口 +
/// 方法名分发（与 v4 host_plugin_call 的 service/method 协议一致），跨 ABI
/// 只交换指针/长度，规避各平台调用约定差异。
#[repr(C)]
pub struct HostVTable {
    /// 日志：log(level, ptr, len)。level: 0=info 1=warn。
    pub log: unsafe extern "C" fn(level: u8, ptr: *const u8, len: usize),
    /// 时间戳（毫秒）。
    pub time_millis: unsafe extern "C" fn() -> i64,
    /// 通用宿主调用：call(ctx, method, payload) -> HostBuffer，NULL 表示失败。
    pub call: unsafe extern "C" fn(
        ctx: *const HostContext,
        method: *const u8,
        method_len: usize,
        payload: *const u8,
        payload_len: usize,
    ) -> *mut HostBuffer,
    /// 归还 call 返回的缓冲。
    pub free_response: unsafe extern "C" fn(buffer: *mut HostBuffer),
}

/// 宿主分配的响应缓冲（call 的返回值载体）。
#[repr(C)]
pub struct HostBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

/// 传给每次调用的上下文：插件名 + 能力表。
#[repr(C)]
pub struct HostContext {
    pub plugin: *const u8,
    pub plugin_len: usize,
    pub vtable: *const HostVTable,
}

/// 宿主服务集合（v4 PluginHostServices 迁移）。
#[derive(Debug, Default)]
pub(crate) struct PluginHostServices {
    pub(super) economy: Mutex<EconomyState>,
    economy_async: Mutex<EconomyAsyncState>,
    structured_storage: Mutex<StructuredStorageState>,
    structured_storage_async: Mutex<StructuredStorageAsyncState>,
    plugin_services: Mutex<PluginServiceState>,
    plugin_api: Mutex<Option<Arc<dyn PluginApiService>>>,
    pathfinding: Mutex<Option<Arc<dyn PathfindingService>>>,
    world_edit: Mutex<Option<Arc<dyn WorldEditService>>>,
    entity_control: Mutex<Option<Arc<dyn EntityControlService>>>,
    random_pools: Mutex<RandomPoolState>,
    geyser_players: Mutex<GeyserPlayerState>,
    /// 本地化查询回调（v4 的 crate::l10n 在 v6 由装配层注入）。
    localizer: Mutex<Option<Arc<dyn LocalizeService>>>,
}

/// 本地化服务（v4 crate::l10n::localize / localize_entity / localize_enchantment）。
/// v6 l10n 模块尚未迁移，由装配层注入实现；未注入时 found=false。
pub trait LocalizeService: Send + Sync + std::fmt::Debug {
    fn localize(&self, key: &str, language: &str, kind: &str) -> Option<String>;
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
pub(crate) enum StructuredStorageOperation {
    Exists,
    Get,
    Set(Vec<u8>),
    Delete,
}

#[derive(Debug)]
pub(crate) enum PollStructuredStorageResult {
    Pending,
    Ready(Vec<u8>),
    Failed,
}

pub trait PluginApiService: Send + Sync + std::fmt::Debug {
    fn call(&self, caller: &str, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>>;
}

#[derive(Debug, Default)]
struct PluginServiceState {
    available_plugins: HashSet<String>,
    providers: HashMap<String, String>,
}

#[derive(Debug, Default)]
struct GeyserPlayerState {
    by_uuid: HashMap<String, GeyserPlayerInfoResponse>,
    username_to_uuid: HashMap<String, String>,
}

pub trait PathfindingService: Send + Sync + std::fmt::Debug {
    fn find_path(&self, query: &str) -> Option<String>;
}

pub trait WorldEditService: Send + Sync + std::fmt::Debug {
    fn set_block(&self, query: &str) -> i32;
    fn set_blocks(&self, query: &str) -> i32;
    fn break_block(&self, query: &str) -> i32;
    fn register_region(&self, query: &str) -> i32;
}

pub trait EntityControlService: Send + Sync + std::fmt::Debug {
    fn upsert(&self, plugin_name: &str, query: &str) -> i32;
    fn move_entity(&self, plugin_name: &str, query: &str) -> i32;
    fn move_npc(&self, query: &str) -> i32;
    fn remove(&self, plugin_name: &str, query: &str) -> i32;
}

impl PluginHostServices {
    pub(crate) fn configure_economy(&self, config: &crate::config::EconomyConfig) {
        self.economy
            .lock()
            .expect("plugin economy state poisoned")
            .configure(config);
    }

    pub(crate) fn configure_structured_storage(
        &self,
        config: &crate::config::StructuredStorageConfig,
    ) {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .configure(config);
    }

    pub(crate) fn set_localizer(&self, localizer: Arc<dyn LocalizeService>) {
        *self.localizer.lock().expect("plugin localizer poisoned") = Some(localizer);
    }

    pub(super) fn structured_storage_exists_of(&self, plugin: &str, key: &str) -> bool {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .exists(plugin, key)
    }

    pub(super) fn structured_storage_get_of(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .get(plugin, key)
    }

    pub(super) fn structured_storage_set_of(&self, plugin: &str, key: &str, value: &[u8]) -> bool {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .set(plugin, key, value)
    }

    pub(super) fn structured_storage_delete_of(&self, plugin: &str, key: &str) -> bool {
        self.structured_storage
            .lock()
            .expect("plugin structured storage state poisoned")
            .delete(plugin, key)
    }

    pub(super) fn submit_structured_storage_async_of(
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
                    services.structured_storage_exists_of(&plugin, &key),
                )]),
                StructuredStorageOperation::Get => services.structured_storage_get_of(&plugin, &key),
                StructuredStorageOperation::Set(value) => Some(vec![u8::from(
                    services.structured_storage_set_of(&plugin, &key, &value),
                )]),
                StructuredStorageOperation::Delete => Some(vec![u8::from(
                    services.structured_storage_delete_of(&plugin, &key),
                )]),
            };
            let _ = sender.send(result);
        });
        id
    }

    pub(super) fn poll_structured_storage_async_of(&self, id: i64) -> PollStructuredStorageResult {
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

    pub(super) fn forget_structured_storage_async_of(&self, id: i64) -> i32 {
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

    pub(crate) fn set_plugin_services(
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

    pub(crate) fn set_plugin_api(&self, plugin_api: Arc<dyn PluginApiService>) {
        *self.plugin_api.lock().expect("plugin API service poisoned") = Some(plugin_api);
    }

    pub(super) fn has_plugin_service_of(&self, service: &str) -> bool {
        self.plugin_services
            .lock()
            .expect("plugin service state poisoned")
            .providers
            .contains_key(service.trim())
    }

    pub(super) fn call_plugin_api_of(
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

    pub(crate) fn upsert_geyser_player(&self, info: GeyserPlayerInfoResponse) {
        let uuid = info.java_uuid.trim().to_string();
        if uuid.is_empty() {
            return;
        }
        let mut state = self
            .geyser_players
            .lock()
            .expect("geyser player state poisoned");
        if !info.username.trim().is_empty() {
            state
                .username_to_uuid
                .insert(info.username.to_ascii_lowercase(), uuid.clone());
        }
        state.by_uuid.insert(uuid, info);
    }

    pub(crate) fn remove_geyser_player(&self, uuid: &str, username: &str) {
        let mut state = self
            .geyser_players
            .lock()
            .expect("geyser player state poisoned");
        state.by_uuid.remove(uuid.trim());
        if !username.trim().is_empty() {
            state
                .username_to_uuid
                .remove(&username.to_ascii_lowercase());
        }
    }

    fn geyser_player_info_of(&self, query: &GeyserPlayerInfoQuery) -> GeyserPlayerInfoResponse {
        let state = self
            .geyser_players
            .lock()
            .expect("geyser player state poisoned");
        if let Some(info) = (!query.uuid.trim().is_empty())
            .then(|| state.by_uuid.get(query.uuid.trim()))
            .flatten()
        {
            return info.clone();
        }
        if let Some(uuid) = (!query.username.trim().is_empty())
            .then(|| {
                state
                    .username_to_uuid
                    .get(&query.username.to_ascii_lowercase())
            })
            .flatten()
            && let Some(info) = state.by_uuid.get(uuid)
        {
            return info.clone();
        }
        GeyserPlayerInfoResponse::default()
    }

    pub(crate) fn set_pathfinding(&self, pathfinding: Arc<dyn PathfindingService>) {
        *self
            .pathfinding
            .lock()
            .expect("pathfinding service poisoned") = Some(pathfinding);
    }

    pub(super) fn find_path_of(&self, query: &str) -> Option<String> {
        self.pathfinding
            .lock()
            .expect("pathfinding service poisoned")
            .as_ref()
            .and_then(|service| service.find_path(query))
    }

    pub(crate) fn set_world_edit(&self, world_edit: Arc<dyn WorldEditService>) {
        *self.world_edit.lock().expect("world edit service poisoned") = Some(world_edit);
    }

    fn set_block_of(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.set_block(query))
            .unwrap_or(-1)
    }

    fn set_blocks_of(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.set_blocks(query))
            .unwrap_or(-1)
    }

    fn break_block_of(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.break_block(query))
            .unwrap_or(-1)
    }

    fn register_region_of(&self, query: &str) -> i32 {
        self.world_edit
            .lock()
            .expect("world edit service poisoned")
            .as_ref()
            .map(|service| service.register_region(query))
            .unwrap_or(-1)
    }

    pub(crate) fn set_entity_control(&self, entity_control: Arc<dyn EntityControlService>) {
        *self
            .entity_control
            .lock()
            .expect("entity control service poisoned") = Some(entity_control);
    }

    fn entity_upsert_of(&self, plugin_name: &str, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.upsert(plugin_name, query))
            .unwrap_or(-1)
    }

    fn entity_move_of(&self, plugin_name: &str, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.move_entity(plugin_name, query))
            .unwrap_or(-1)
    }

    fn npc_move_of(&self, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.move_npc(query))
            .unwrap_or(-1)
    }

    fn entity_remove_of(&self, plugin_name: &str, query: &str) -> i32 {
        self.entity_control
            .lock()
            .expect("entity control service poisoned")
            .as_ref()
            .map(|service| service.remove(plugin_name, query))
            .unwrap_or(-1)
    }

    pub(super) fn submit_economy_async_of(
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

    pub(super) fn poll_economy_async_of(&self, id: i64) -> i64 {
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

    pub(super) fn forget_economy_async_of(&self, id: i64) -> i32 {
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

/// 随机池（v4 RandomPoolState 迁移）：加权随机 + 预计算缓存。
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

#[derive(Debug, Clone, Copy)]
pub(super) enum EconomyUpdate {
    Balance,
    Set,
    Deposit,
    Withdraw,
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
    let Some(currency) = economy.normalize_existing_currency(currency) else {
        return i64::MIN;
    };
    let result = match update {
        EconomyUpdate::Balance => economy.balance(player, &currency),
        EconomyUpdate::Set => economy.set_balance(player, &currency, amount),
        EconomyUpdate::Deposit => economy.deposit(player, &currency, amount),
        EconomyUpdate::Withdraw => match economy.withdraw(player, &currency, amount).flatten() {
            Some(balance) => Some(balance),
            None => return i64::MIN + 1,
        },
    };
    result.unwrap_or(i64::MIN)
}

/// 全局宿主服务（HostVTable 回调的数据源；由 PluginManager 装配时设置）。
static HOST_SERVICES: std::sync::OnceLock<Arc<PluginHostServices>> = std::sync::OnceLock::new();

pub(super) fn set_host_services(services: Arc<PluginHostServices>) {
    let _ = HOST_SERVICES.set(services);
}

fn host_services() -> Option<Arc<PluginHostServices>> {
    HOST_SERVICES.get().cloned()
}

/// 宿主 vtable 实现（unsafe extern "C" 回调，跨 ABI 边界）。
pub(crate) const HOST_VTABLE: HostVTable = HostVTable {
    log: host_log_ffi,
    time_millis: host_time_millis_ffi,
    call: host_call_ffi,
    free_response: host_free_response_ffi,
};

/// 安全切片读取：空指针 + 长度合法（0 长度返回空切片）。
unsafe fn read_bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return (len == 0).then_some(&[][..]);
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

unsafe extern "C" fn host_log_ffi(level: u8, ptr: *const u8, len: usize) {
    let Some(bytes) = (unsafe { read_bytes(ptr, len) }) else {
        return;
    };
    if bytes.len() > MAX_HOST_LOG_BYTES {
        return;
    }
    match std::str::from_utf8(bytes) {
        Ok(message) => log::info!("[plugin] {message}"),
        Err(_) if level == 1 => log::warn!("[plugin] <non-utf8 log message>"),
        Err(_) => {}
    }
}

unsafe extern "C" fn host_time_millis_ffi() -> i64 {
    host_time_millis()
}

pub(super) fn host_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

unsafe extern "C" fn host_free_response_ffi(buffer: *mut HostBuffer) {
    if buffer.is_null() {
        return;
    }
    unsafe {
        let host_buffer = Box::from_raw(buffer);
        if !host_buffer.ptr.is_null() {
            drop(Vec::from_raw_parts(host_buffer.ptr, host_buffer.len, host_buffer.len));
        }
    }
}

/// 把响应字节打包成宿主缓冲（NULL 表示分配失败）。
fn into_host_buffer(bytes: Vec<u8>) -> *mut HostBuffer {
    let mut bytes = std::mem::ManuallyDrop::new(bytes);
    let ptr = bytes.as_mut_ptr();
    let len = bytes.len();
    Box::into_raw(Box::new(HostBuffer { ptr, len }))
}

unsafe extern "C" fn host_call_ffi(
    ctx: *const HostContext,
    method: *const u8,
    method_len: usize,
    payload: *const u8,
    payload_len: usize,
) -> *mut HostBuffer {
    if ctx.is_null() {
        return std::ptr::null_mut();
    }
    let plugin_name = unsafe { read_bytes((*ctx).plugin, (*ctx).plugin_len) }
        .and_then(|bytes| std::str::from_utf8(bytes).ok().map(str::to_string))
        .unwrap_or_default();
    let Some(method) = (unsafe { read_bytes(method, method_len) })
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
    else {
        return std::ptr::null_mut();
    };
    let Some(payload) = (unsafe { read_bytes(payload, payload_len) }) else {
        return std::ptr::null_mut();
    };

    let Some(services) = host_services() else {
        return std::ptr::null_mut();
    };

    match host_dispatch(&services, &plugin_name, method, payload) {
        Some(bytes) => into_host_buffer(bytes),
        None => std::ptr::null_mut(),
    }
}

/// 宿主方法分发：方法名与 v4 WASM host 函数一一对应。
/// 返回 Some(bytes) 写回插件；数值类结果编码为 ASCII 字符串（i32/i64 十进制，
/// 布尔 0/1），结构体类结果编码为 JSON。
fn host_dispatch(
    services: &Arc<PluginHostServices>,
    plugin: &str,
    method: &str,
    payload: &[u8],
) -> Option<Vec<u8>> {
    match method {
        // ---- 配置文件（v4 host_config_*）----
        "config_exists" => {
            let path = host_string(payload, MAX_HOST_PATH_BYTES)?;
            Some(i32::from(plugin_config_path(plugin, &path)?.is_file()).to_string().into_bytes())
        }
        "config_read" => {
            let path = host_string(payload, MAX_HOST_PATH_BYTES)?;
            let bytes = std::fs::read(plugin_config_path(plugin, &path)?).ok()?;
            (bytes.len() <= MAX_HOST_CONFIG_BYTES).then_some(bytes)
        }
        "config_write" => {
            // payload = path \0 data
            let (path, data) = split_zero(payload, MAX_HOST_PATH_BYTES)?;
            let path = std::str::from_utf8(path).ok()?;
            (data.len() <= MAX_HOST_CONFIG_BYTES).then_some(())?;
            let path = plugin_config_path(plugin, path)?;
            let parent = path.parent()?;
            std::fs::create_dir_all(parent).ok()?;
            std::fs::write(path, data).ok()?;
            Some(b"0".to_vec())
        }
        // ---- 键值存储（v4 host_storage_*）----
        "storage_exists" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            Some(i32::from(plugin_storage_path(plugin, &key)?.is_file()).to_string().into_bytes())
        }
        "storage_get" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            let bytes = std::fs::read(plugin_storage_path(plugin, &key)?).ok()?;
            (bytes.len() <= MAX_HOST_STORAGE_VALUE_BYTES).then_some(bytes)
        }
        "storage_set" => {
            let (key, data) = split_zero(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            let key = std::str::from_utf8(key).ok()?;
            (data.len() <= MAX_HOST_STORAGE_VALUE_BYTES).then_some(())?;
            let path = plugin_storage_path(plugin, key)?;
            let parent = path.parent()?;
            std::fs::create_dir_all(parent).ok()?;
            std::fs::write(path, data).ok()?;
            Some(b"0".to_vec())
        }
        "storage_delete" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            let path = plugin_storage_path(plugin, &key)?;
            match std::fs::remove_file(path) {
                Ok(()) => Some(b"0".to_vec()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Some(b"0".to_vec()),
                Err(_) => None,
            }
        }
        // ---- 结构化存储同步（v4 host_structured_storage_*）----
        "structured_storage_exists" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            Some(vec![u8::from(services.structured_storage_exists_of(plugin, &key))])
        }
        "structured_storage_get" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            let bytes = services.structured_storage_get_of(plugin, &key)?;
            (bytes.len() <= MAX_HOST_STORAGE_VALUE_BYTES).then_some(bytes)
        }
        "structured_storage_set" => {
            let (key, data) = split_zero(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            let key = std::str::from_utf8(key).ok()?;
            (data.len() <= MAX_HOST_STORAGE_VALUE_BYTES).then_some(())?;
            let data = data.to_vec();
            i32::from(services.structured_storage_set_of(plugin, key, &data))
                .to_string()
                .into_bytes()
                .into()
        }
        "structured_storage_delete" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            i32::from(services.structured_storage_delete_of(plugin, &key))
                .to_string()
                .into_bytes()
                .into()
        }
        // ---- 结构化存储异步（v4 *_async / poll / forget）----
        "structured_storage_exists_async" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            Some(
                services
                    .submit_structured_storage_async_of(plugin.to_string(), key, StructuredStorageOperation::Exists)
                    .to_string()
                    .into_bytes(),
            )
        }
        "structured_storage_get_async" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            Some(
                services
                    .submit_structured_storage_async_of(plugin.to_string(), key, StructuredStorageOperation::Get)
                    .to_string()
                    .into_bytes(),
            )
        }
        "structured_storage_set_async" => {
            let (key, data) = split_zero(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            let key = std::str::from_utf8(key).ok()?.to_string();
            (data.len() <= MAX_HOST_STORAGE_VALUE_BYTES).then_some(())?;
            Some(
                services
                    .submit_structured_storage_async_of(plugin.to_string(), key, StructuredStorageOperation::Set(data.to_vec()))
                    .to_string()
                    .into_bytes(),
            )
        }
        "structured_storage_delete_async" => {
            let key = host_string(payload, MAX_HOST_STORAGE_KEY_BYTES)?;
            Some(
                services
                    .submit_structured_storage_async_of(plugin.to_string(), key, StructuredStorageOperation::Delete)
                    .to_string()
                    .into_bytes(),
            )
        }
        "structured_storage_async_poll" => {
            // payload = ASCII i64 id
            let id = host_string(payload, 32)?;
            let id: i64 = id.trim().parse().ok()?;
            match services.poll_structured_storage_async_of(id) {
                PollStructuredStorageResult::Pending => Some(STRUCTURED_STORAGE_ASYNC_PENDING.to_string().into_bytes()),
                PollStructuredStorageResult::Ready(bytes) => Some(bytes),
                PollStructuredStorageResult::Failed => None,
            }
        }
        "structured_storage_async_forget" => {
            let id = host_string(payload, 32)?;
            let id: i64 = id.trim().parse().ok()?;
            Some(services.forget_structured_storage_async_of(id).to_string().into_bytes())
        }
        // ---- 经济（v4 host_economy_*；数值结果为 ASCII，失败 i64::MIN）----
        "economy_register_currency" => {
            // payload = id \0 name \0 symbol \0 fractional_digits
            let mut fields = payload.split(|&b| b == 0);
            let id = std::str::from_utf8(fields.next()?).ok()?;
            let name = std::str::from_utf8(fields.next()?).ok()?.to_string();
            let symbol = std::str::from_utf8(fields.next()?).ok()?.to_string();
            let digits = std::str::from_utf8(fields.next()?).ok()?.trim().parse::<i32>().ok()?;
            let id = normalize_currency(id);
            if id.is_empty() {
                return None;
            }
            services
                .economy
                .lock()
                .expect("plugin economy state poisoned")
                .register_currency(id, name, symbol, digits);
            Some(b"0".to_vec())
        }
        "economy_currency_info" => {
            let currency = host_string(payload, MAX_HOST_CURRENCY_BYTES)?;
            let info = services
                .economy
                .lock()
                .expect("plugin economy state poisoned")
                .currency_info(&currency)?;
            Some(format!("{}\t{}\t{}\t{}", info.id, info.name, info.symbol, info.fractional_digits).into_bytes())
        }
        "economy_storage" => {
            let currency = host_string(payload, MAX_HOST_CURRENCY_BYTES)?;
            let storage = services
                .economy
                .lock()
                .expect("plugin economy state poisoned")
                .storage_for(&currency);
            Some(storage.into_bytes())
        }
        "economy_balance" | "economy_set_balance" | "economy_deposit" | "economy_withdraw" => {
            // payload = player \0 currency [\0 amount]
            let update = match method {
                "economy_balance" => EconomyUpdate::Balance,
                "economy_set_balance" => EconomyUpdate::Set,
                "economy_deposit" => EconomyUpdate::Deposit,
                _ => EconomyUpdate::Withdraw,
            };
            let result = economy_call(services, payload, update)?;
            Some(result.to_string().into_bytes())
        }
        "economy_balance_async" | "economy_set_balance_async" | "economy_deposit_async" | "economy_withdraw_async" => {
            let update = match method {
                "economy_balance_async" => EconomyUpdate::Balance,
                "economy_set_balance_async" => EconomyUpdate::Set,
                "economy_deposit_async" => EconomyUpdate::Deposit,
                _ => EconomyUpdate::Withdraw,
            };
            let (player, currency, amount) = economy_args(payload)?;
            let id = services.submit_economy_async_of(player, currency, amount, update);
            Some(id.to_string().into_bytes())
        }
        "economy_async_poll" => {
            let id = host_string(payload, 32)?.trim().parse::<i64>().ok()?;
            Some(services.poll_economy_async_of(id).to_string().into_bytes())
        }
        "economy_async_forget" => {
            let id = host_string(payload, 32)?.trim().parse::<i64>().ok()?;
            Some(services.forget_economy_async_of(id).to_string().into_bytes())
        }
        // ---- 抽奖 / 随机池（v4 host_lottery_roll / host_random_pool_roll）----
        "lottery_roll" => {
            let entries = host_string(payload, MAX_HOST_CONFIG_BYTES)?;
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
                return None;
            }
            let mut pick = rand::Rng::gen_range(&mut rand::thread_rng(), 0..total);
            for (weight, value) in parsed {
                if pick < weight {
                    return Some(value.into_bytes());
                }
                pick -= weight;
            }
            None
        }
        "random_pool_roll" => {
            let request = host_string(payload, MAX_HOST_RANDOM_POOL_BYTES)?;
            let value = services
                .random_pools
                .lock()
                .expect("plugin random pool state poisoned")
                .roll(&request)?;
            Some(value.into_bytes())
        }
        // ---- 寻路 / world edit / 实体控制（v4 host_*，ASCII i32 结果）----
        "pathfinding_find" => {
            let query = host_string(payload, MAX_HOST_PATHFINDING_BYTES)?;
            services.find_path_of(&query).map(String::into_bytes)
        }
        "geyser_player_info" => {
            let query: GeyserPlayerInfoQuery = serde_json::from_slice(payload).ok()?;
            let response = services.geyser_player_info_of(&query);
            serde_json::to_vec(&response).ok()
        }
        "world_set_block" => {
            let query = host_string(payload, MAX_HOST_WORLD_EDIT_BYTES)?;
            Some(services.set_block_of(&query).to_string().into_bytes())
        }
        "world_set_blocks" => {
            let query = host_string(payload, MAX_HOST_WORLD_EDIT_BATCH_BYTES)?;
            Some(services.set_blocks_of(&query).to_string().into_bytes())
        }
        "world_break_block" => {
            let query = host_string(payload, MAX_HOST_WORLD_EDIT_BYTES)?;
            Some(services.break_block_of(&query).to_string().into_bytes())
        }
        "world_register_edit_region" => {
            let query = host_string(payload, MAX_HOST_WORLD_EDIT_BYTES)?;
            Some(services.register_region_of(&query).to_string().into_bytes())
        }
        "entity_upsert" => {
            let query = host_string(payload, MAX_HOST_ENTITY_CONTROL_BYTES)?;
            Some(services.entity_upsert_of(plugin, &query).to_string().into_bytes())
        }
        "entity_move" => {
            let query = host_string(payload, MAX_HOST_ENTITY_CONTROL_BYTES)?;
            Some(services.entity_move_of(plugin, &query).to_string().into_bytes())
        }
        "npc_move" => {
            let query = host_string(payload, MAX_HOST_ENTITY_CONTROL_BYTES)?;
            Some(services.npc_move_of(&query).to_string().into_bytes())
        }
        "entity_remove" => {
            let query = host_string(payload, MAX_HOST_ENTITY_CONTROL_BYTES)?;
            Some(services.entity_remove_of(plugin, &query).to_string().into_bytes())
        }
        // ---- 插件间调用（v4 host_plugin_service_exists / host_plugin_call）----
        "plugin_service_exists" => {
            let service = host_string(payload, 256)?;
            Some(i32::from(services.has_plugin_service_of(&service)).to_string().into_bytes())
        }
        "plugin_call" => {
            // payload = service \0 method \0 body
            let (service, rest) = split_zero(payload, 256)?;
            let (method, body) = split_zero(rest, 256)?;
            let service = std::str::from_utf8(service).ok()?;
            let method = std::str::from_utf8(method).ok()?;
            (body.len() <= MAX_HOST_PLUGIN_API_BYTES).then_some(())?;
            services.call_plugin_api_of(plugin, service, method, body)
        }
        // ---- HTTP（v4 host_http_request；v6 无 reqwest 依赖 → 暂不支持）----
        "http_request" => {
            // TODO(http): v6 workspace 未引入 HTTP 客户端依赖，
            // 返回结构化错误（status=0 + error 文案）保持 v4 协议形状。
            let _request: crate::api::HttpRequest = serde_json::from_slice(payload).ok()?;
            let response = HttpResponse {
                status: 0,
                headers: Vec::new(),
                body: Vec::new(),
                error: "http client unavailable in this build".to_string(),
            };
            serde_json::to_vec(&response).ok()
        }
        // ---- 本地化（v4 host_localize）----
        "localize" => {
            let query: crate::api::LocalizedNameQuery = serde_json::from_slice(payload).ok()?;
            let name = services
                .localizer
                .lock()
                .expect("plugin localizer poisoned")
                .as_ref()
                .and_then(|service| service.localize(&query.key, &query.language, &query.kind));
            let response = crate::api::LocalizedNameResponse {
                name: name.clone().unwrap_or_default(),
                found: name.is_some(),
            };
            serde_json::to_vec(&response).ok()
        }
        _ => None,
    }
}

fn economy_call(
    services: &Arc<PluginHostServices>,
    payload: &[u8],
    update: EconomyUpdate,
) -> Option<i64> {
    let (player, currency, amount) = economy_args(payload)?;
    let economy = services
        .economy
        .lock()
        .expect("plugin economy state poisoned");
    let Some(currency) = economy.normalize_existing_currency(&currency) else {
        return Some(i64::MIN);
    };
    let result = match update {
        EconomyUpdate::Balance => economy.balance(&player, &currency),
        EconomyUpdate::Set => economy.set_balance(&player, &currency, amount),
        EconomyUpdate::Deposit => economy.deposit(&player, &currency, amount),
        EconomyUpdate::Withdraw => economy.withdraw(&player, &currency, amount).flatten(),
    };
    Some(result.unwrap_or(i64::MIN))
}

fn economy_args(payload: &[u8]) -> Option<(String, String, i64)> {
    let mut fields = payload.split(|&b| b == 0);
    let player = std::str::from_utf8(fields.next()?).ok()?.trim().to_string();
    let currency = std::str::from_utf8(fields.next()?).ok()?.trim().to_string();
    let amount = fields
        .next()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| text.trim().parse::<i64>().ok())
        .unwrap_or(0);
    if player.is_empty() || amount < 0 {
        return None;
    }
    Some((player, currency, amount))
}

fn host_string(payload: &[u8], max_len: usize) -> Option<String> {
    (payload.len() <= max_len)
        .then_some(())?;
    std::str::from_utf8(payload).ok().map(str::to_string)
}

/// 按 NUL 拆分两段（首段限制 max_len）。
fn split_zero(payload: &[u8], max_len: usize) -> Option<(&[u8], &[u8])> {
    let idx = payload.iter().position(|&b| b == 0)?;
    let (head, rest) = payload.split_at(idx);
    (head.len() <= max_len).then_some(())?;
    Some((head, &rest[1..]))
}

/// 插件配置文件路径（v4 plugin_config_path 迁移）：config/plugins/<plugin>/<relative>。
fn plugin_config_path(plugin_name: &str, requested: &str) -> Option<std::path::PathBuf> {
    use std::path::{Component, Path, PathBuf};
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

/// 插件键值存储路径（v4 plugin_storage_path 迁移）：config/plugins/<plugin>/storage/<key>.bin。
fn plugin_storage_path(plugin_name: &str, key: &str) -> Option<std::path::PathBuf> {
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

fn clean_storage_key(key: &str) -> Option<std::path::PathBuf> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    let mut clean = std::path::PathBuf::new();
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

    #[test]
    fn split_zero_splits_head_and_rest() {
        let payload = b"plugins/a.toml\0data-bytes";
        let (head, rest) = split_zero(payload, 1024).unwrap();
        assert_eq!(head, b"plugins/a.toml".as_slice());
        assert_eq!(rest, b"data-bytes".as_slice());
    }

    #[test]
    fn clean_storage_key_rejects_traversal() {
        assert!(clean_storage_key("../etc/passwd").is_none());
        assert!(clean_storage_key("").is_none());
        assert_eq!(
            clean_storage_key("player/data").unwrap().to_str().unwrap(),
            "player\\data.bin"
        );
    }
}

