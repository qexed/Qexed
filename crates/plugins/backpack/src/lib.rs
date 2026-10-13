//! backpack：v4 移植 —— 虚拟背包 + 跨插件服务 qexed.backpack.v1。
//!
//! v4：player_item_pickup / config_reload / api_call 导出 + manifest services。
//! v6：PlayerItemPickupEvent（休眠就绪）+ ConfigReloadEvent +
//!     plugin_register_service(PluginServiceProvider)。服务经全局 ActionHub 直读
//!     本插件存储；配置在 enable 时缓存，ConfigReloadEvent 时刷新。

use qexed_plugin_api::services::PluginServiceProvider;
use qexed_plugin_api::{ConfigReloadEvent, LogLevel, PlayerItemPickupEvent};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::RwLock;

const PLUGIN_ID: &str = "qexed.backpack";
const BACKPACK_SERVICE: &str = "qexed.backpack.v1";
const PICKUP_MESSAGE_COOLDOWN_MS: i64 = 2000;

// ── 配置（enable 时加载缓存，reload 时刷新） ─────────────────────────────

static CONFIG: RwLock<Option<BackpackConfig>> = RwLock::new(None);

fn cached_config() -> BackpackConfig {
    CONFIG.read().unwrap().clone().unwrap_or_default()
}

#[derive(Debug, Clone, Deserialize)]
struct BackpackConfig {
    #[serde(default = "default_enable")]
    enable: bool,
    /// "disabled" | "whitelist" | "blacklist"
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    items: Vec<BackpackItemConfig>,
    #[serde(default = "default_enable")]
    pickup_message_enable: bool,
    #[serde(default = "default_pickup_message")]
    pickup_message: String,
    #[serde(default)]
    pickup_message_overlay: bool,
}
impl Default for BackpackConfig {
    fn default() -> Self {
        serde_json::from_str(
            &serde_json::to_string(&ConfigSeed::default()).unwrap(),
        )
        .unwrap()
    }
}
/// 全 Default 种子（驱动 serde 默认值）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ConfigSeed {
    #[serde(default = "default_enable")]
    enable: bool,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    items: Vec<BackpackItemConfig>,
    #[serde(default = "default_enable")]
    pickup_message_enable: bool,
    #[serde(default = "default_pickup_message")]
    pickup_message: String,
    #[serde(default)]
    pickup_message_overlay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackpackItemConfig {
    item: String,
    /// -1 = infinite, 0/positive = limited
    #[serde(default = "default_infinite_capacity")]
    capacity: i64,
    #[serde(default = "default_enable")]
    sellable: bool,
}

impl BackpackConfig {
    fn can_pickup(&self, item_name: &str) -> bool {
        match self.mode.as_str() {
            "whitelist" => self.items.iter().any(|e| e.item.eq_ignore_ascii_case(item_name)),
            "blacklist" => !self.items.iter().any(|e| e.item.eq_ignore_ascii_case(item_name)),
            _ => true,
        }
    }

    fn item_capacity(&self, item_name: &str) -> i64 {
        self.items
            .iter()
            .find(|e| e.item.eq_ignore_ascii_case(item_name))
            .map(|e| e.capacity)
            .unwrap_or(-1)
    }
}

// ── 背包数据模型 ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PlayerBackpack {
    /// item_name → count
    items: BTreeMap<String, i64>,
}

impl PlayerBackpack {
    fn total_items(&self) -> i64 {
        self.items.values().sum()
    }

    fn add(&mut self, item_name: &str, count: i64, capacity: i64) -> i64 {
        if count <= 0 {
            return self.count(item_name);
        }
        let key = item_name.trim().to_lowercase();
        let current = self.items.get(&key).copied().unwrap_or(0);
        let new_count = if capacity < 0 {
            current.saturating_add(count)
        } else {
            (current.saturating_add(count)).min(capacity)
        };
        let added = new_count - current;
        if new_count > 0 {
            self.items.insert(key, new_count);
        } else {
            self.items.remove(key.as_str());
        }
        added
    }

    fn remove(&mut self, item_name: &str, count: i64) -> i64 {
        if count <= 0 {
            return self.count(item_name);
        }
        let key = item_name.trim().to_lowercase();
        let current = self.items.get(&key).copied().unwrap_or(0);
        let new_count = current.saturating_sub(count);
        if new_count > 0 {
            self.items.insert(key.clone(), new_count);
        } else {
            self.items.remove(&key);
        }
        current - new_count
    }

    fn count(&self, item_name: &str) -> i64 {
        self.items
            .get(item_name.trim().to_lowercase().as_str())
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PickupMessageState {
    last_message_ms: i64,
}

// ── 存储（经全局 ActionHub，键仅用合法字符） ─────────────────────────────

fn backpack_key(uuid: &str) -> String {
    format!("backpack/{uuid}")
}

fn cooldown_key(uuid: &str) -> String {
    format!("pickup_msg_cooldown/{uuid}")
}

fn hub() -> &'static qexed_plugin::host_api::action::ActionHub {
    qexed_plugin::action_hub()
}

fn load_backpack(uuid: &str) -> PlayerBackpack {
    hub()
        .storage_get(PLUGIN_ID, &backpack_key(uuid))
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_backpack(uuid: &str, backpack: &PlayerBackpack) {
    if let Ok(bytes) = serde_json::to_vec(backpack) {
        hub().storage_set(PLUGIN_ID, &backpack_key(uuid), &bytes);
    }
}

// ── 跨插件服务 ──────────────────────────────────────────────────────────

struct BackpackService;

#[derive(Debug, Serialize)]
struct GetItemsResponse {
    items: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize)]
struct CountResponse {
    item: String,
    count: i64,
}

#[derive(Deserialize)]
struct CountReq {
    player_uuid: String,
    item: String,
}

#[derive(Deserialize)]
struct AddReq {
    player_uuid: String,
    item: String,
    count: i64,
}

impl PluginServiceProvider for BackpackService {
    fn call(&self, _caller: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
        let result = match method {
            "get_items" => {
                let uuid: String = serde_json::from_slice(payload).ok()?;
                let backpack = load_backpack(&uuid);
                serde_json::to_vec(&GetItemsResponse { items: backpack.items }).ok()
            }
            "get_item_count" => {
                let req: CountReq = serde_json::from_slice(payload).ok()?;
                let count = load_backpack(&req.player_uuid).count(&req.item);
                serde_json::to_vec(&CountResponse { item: req.item, count }).ok()
            }
            "add_item" => {
                let req: AddReq = serde_json::from_slice(payload).ok()?;
                let capacity = cached_config().item_capacity(&req.item);
                let mut backpack = load_backpack(&req.player_uuid);
                let _ = backpack.add(&req.item, req.count, capacity);
                save_backpack(&req.player_uuid, &backpack);
                let count = backpack.count(&req.item);
                serde_json::to_vec(&CountResponse { item: req.item, count }).ok()
            }
            "remove_item" => {
                let req: AddReq = serde_json::from_slice(payload).ok()?;
                let mut backpack = load_backpack(&req.player_uuid);
                backpack.remove(&req.item, req.count);
                save_backpack(&req.player_uuid, &backpack);
                let count = backpack.count(&req.item);
                serde_json::to_vec(&CountResponse { item: req.item, count }).ok()
            }
            "remove_items" => {
                #[derive(Deserialize)]
                struct Entry {
                    item: String,
                    count: i64,
                }
                #[derive(Deserialize)]
                struct MultiReq {
                    player_uuid: String,
                    items: Vec<Entry>,
                }
                let req: MultiReq = serde_json::from_slice(payload).ok()?;
                let mut backpack = load_backpack(&req.player_uuid);
                let mut removed = BTreeMap::new();
                for entry in &req.items {
                    let removed_count = backpack.remove(&entry.item, entry.count);
                    if removed_count > 0 {
                        removed.insert(entry.item.clone(), removed_count);
                    }
                }
                save_backpack(&req.player_uuid, &backpack);
                serde_json::to_vec(&GetItemsResponse { items: removed }).ok()
            }
            _ => None,
        };
        result
    }
}

// ── 生命周期 ────────────────────────────────────────────────────────────

qexed_plugin_sdk::declare_simple_plugin!("qexed.backpack", "Backpack", "0.2.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        if let Ok(Some(text)) = sdk.read_config() {
            if let Ok(doc) = text.parse::<toml_edit::DocumentMut>() {
                if let Ok(cfg) = qexed_toml::from_document::<BackpackConfig>(doc) {
                    *CONFIG.write().unwrap() = Some(cfg);
                }
            }
        }
        let registered = sdk.plugin_register_service(BACKPACK_SERVICE, Arc::new(BackpackService));
        ctx.log(LogLevel::Info, &format!("backpack plugin initialized (service: {registered})"));

        ctx.register::<ConfigReloadEvent>(move |_api, ev, _| {
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("backpack/config.toml") || path.ends_with("config.toml") && path.contains("backpack")) {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(_api, plugin_id);
                if let Ok(Some(text)) = sdk.read_config() {
                    if let Ok(doc) = text.parse::<toml_edit::DocumentMut>() {
                        if let Ok(cfg) = qexed_toml::from_document::<BackpackConfig>(doc) {
                            *CONFIG.write().unwrap() = Some(cfg);
                        }
                    }
                }
            })
        });

        // 拾取入包（宿主拾取管线接线后生效；consume=true 语义待宿主暴露）
        ctx.register::<PlayerItemPickupEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let config = cached_config();
                if !config.enable {
                    return;
                }
                let item_name = ev.item_name.trim();
                if item_name.is_empty() || !config.can_pickup(item_name) {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let mut backpack = load_backpack(&ev.player.uuid);
                let capacity = config.item_capacity(item_name);
                let added = backpack.add(item_name, i64::from(ev.count), capacity);
                save_backpack(&ev.player.uuid, &backpack);

                if config.pickup_message_enable && added > 0 {
                    let now = now_millis();
                    let mut state: PickupMessageState = sdk
                        .storage_get_json(&cooldown_key(&ev.player.uuid))
                        .unwrap_or_default();
                    if now - state.last_message_ms >= PICKUP_MESSAGE_COOLDOWN_MS {
                        state.last_message_ms = now;
                        let _ = sdk.storage_set_json(&cooldown_key(&ev.player.uuid), &state);
                        let total = backpack.total_items();
                        let msg = config
                            .pickup_message
                            .replace("{item}", &format_name(item_name))
                            .replace("{count}", &added.to_string())
                            .replace("{total}", &total.to_string());
                        let action = qexed_plugin_api::payload::PlayerAction::SystemMessage {
                            text: msg,
                            translate: String::new(),
                            with: Vec::new(),
                            overlay: config.pickup_message_overlay,
                        };
                        sdk.perform(&ev.player, action);
                    }
                }
            })
        });
    })
});

fn format_name(item_name: &str) -> String {
    item_name.strip_prefix("minecraft:").unwrap_or(item_name).replace('_', " ")
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn default_enable() -> bool { true }
fn default_mode() -> String { "disabled".to_string() }
fn default_infinite_capacity() -> i64 { -1 }
fn default_pickup_message() -> String { "+{count}x {item} → 背包 ({total})".to_string() }

const DEFAULT_CONFIG: &str = r##"# 虚拟背包配置
enable = true

# 过滤模式: "disabled"(全部拾取) | "whitelist"(白名单) | "blacklist"(黑名单)
mode = "disabled"

# 拾取消息
pickup_message_enable = true
pickup_message = "+{count}x {item} → 背包 ({total})"
pickup_message_overlay = true

# 物品配置
# capacity: -1 = 无限, >0 = 限制数量
# sellable: 是否允许售出

[[items]]
item = "minecraft:cobblestone"
capacity = -1
sellable = true

[[items]]
item = "minecraft:stone"
capacity = -1
sellable = true

[[items]]
item = "minecraft:coal"
capacity = -1
sellable = true

[[items]]
item = "minecraft:diamond"
capacity = 5000
sellable = true

[[items]]
item = "minecraft:emerald"
capacity = 5000
sellable = true

[[items]]
item = "minecraft:ancient_debris"
capacity = 2000
sellable = true

[[items]]
item = "minecraft:netherite_scrap"
capacity = -1
sellable = true
"##;
