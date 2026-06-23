use qexed_plugin_sdk::{
    ConfigReloadPayload, PlayerAction, PlayerItemPickupQuery, PlayerItemPickupResponse,
    PlayerPayload, PluginApiCallQuery, PluginApiCallResponse, PluginManifest,
    PluginServiceDefinition, config_load_or_create, config_read_to_string, storage_get_typed,
    storage_set_typed,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.backpack".to_string(),
    version: "0.2.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    load_after: Vec::new(),
    services: vec![PluginServiceDefinition {
        id: BACKPACK_SERVICE.to_string(),
        version: "0.2.0".to_string(),
        methods: vec![
            "get_items".to_string(),
            "get_item_count".to_string(),
            "add_item".to_string(),
            "remove_item".to_string(),
            "remove_items".to_string(),
        ],
    }],
});

const CONFIG_PATH: &str = "config.toml";
const BACKPACK_SERVICE: &str = "qexed.backpack.v1";
const PICKUP_MESSAGE_COOLDOWN_MS: i64 = 2000;

/// --- Config ----------------------------------------------------------------

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

#[derive(Debug, Clone, Deserialize)]
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
            "whitelist" => self
                .items
                .iter()
                .any(|entry| entry.item.eq_ignore_ascii_case(item_name)),
            "blacklist" => !self
                .items
                .iter()
                .any(|entry| entry.item.eq_ignore_ascii_case(item_name)),
            _ => true, // disabled mode → pickup everything
        }
    }

    fn item_capacity(&self, item_name: &str) -> i64 {
        self.items
            .iter()
            .find(|entry| entry.item.eq_ignore_ascii_case(item_name))
            .map(|entry| entry.capacity)
            .unwrap_or(-1)
    }

    fn is_sellable(&self, item_name: &str) -> bool {
        self.items
            .iter()
            .find(|entry| entry.item.eq_ignore_ascii_case(item_name))
            .map(|entry| entry.sellable)
            .unwrap_or(true) // default sellable
    }
}

/// --- Backpack data model ---------------------------------------------------

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
        let current = self.items.get(item_name).copied().unwrap_or(0);
        let new_count = if capacity < 0 {
            current.saturating_add(count)
        } else {
            (current.saturating_add(count)).min(capacity)
        };
        let added = new_count - current;
        if new_count > 0 {
            self.items
                .insert(item_name.trim().to_lowercase(), new_count);
        } else {
            self.items.remove(item_name.trim().to_lowercase().as_str());
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
            self.items.insert(key, new_count);
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

/// --- Pickup cooldown -------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PickupMessageState {
    last_message_ms: i64,
}

/// --- Init ------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    50
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    qexed_plugin_sdk::log("backpack plugin initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if !path.ends_with("backpack/config.toml") && !path.ends_with(CONFIG_PATH) {
        return;
    }
    qexed_plugin_sdk::log("backpack config reloaded");
}

/// --- Player join / leave ---------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(_payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return;
    };
    // Ensure backpack exists (lazy init, no need to pre-create)
}

/// --- PlayerItemPickup ------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_item_pickup(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerItemPickupQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    };

    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    }

    let item_name = payload.item_name.trim();
    if item_name.is_empty() || !config.can_pickup(item_name) {
        return qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse::default());
    }

    let mut backpack = load_backpack(&payload.player.uuid);
    let capacity = config.item_capacity(item_name);
    let added = backpack.add(item_name, payload.count as i64, capacity);
    save_backpack(&payload.player.uuid, &backpack);

    let mut actions = Vec::new();
    if config.pickup_message_enable && added > 0 {
        let now = qexed_plugin_sdk::time_millis();
        let mut state =
            storage_get_typed::<PickupMessageState>(&cooldown_key(&payload.player.uuid))
                .unwrap_or_default();
        if now - state.last_message_ms >= PICKUP_MESSAGE_COOLDOWN_MS {
            state.last_message_ms = now;
            let _ = storage_set_typed(&cooldown_key(&payload.player.uuid), &state);
            let total = backpack.total_items();
            let msg = config
                .pickup_message
                .replace("{item}", &format_name(item_name))
                .replace("{count}", &added.to_string())
                .replace("{total}", &total.to_string());
            actions.push(PlayerAction::SystemMessage {
                text: msg,
                translate: String::new(),
                with: Vec::new(),
                overlay: config.pickup_message_overlay,
            });
        }
    }

    qexed_plugin_sdk::response_ptr_len(&PlayerItemPickupResponse {
        cancel: false,
        consume: true, // items go to backpack, not inventory
        actions,
    })
}

/// --- BlockDrops: let prison handle blast/furnace, pickup does the rest ----
/// Items drop as entities → PlayerItemPickup intercepts into backpack.
/// This avoids response conflicts with prison_mine_selector's BlockDrops handler.

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_block_drops(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&qexed_plugin_sdk::BlockDropResponse::default())
}

/// --- Plugin API (service: qexed.backpack.v1) --------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_api_call(ptr: i32, len: i32) -> i64 {
    let Some(query) = (unsafe { qexed_plugin_sdk::decode_payload::<PluginApiCallQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&api_error("decode failed"));
    };

    if query.service != BACKPACK_SERVICE {
        return qexed_plugin_sdk::response_ptr_len(&api_error("unknown service"));
    }

    let result = match query.method.as_str() {
        "get_items" => api_get_items(&query),
        "get_item_count" => api_get_item_count(&query),
        "add_item" => api_add_item(&query),
        "remove_item" => api_remove_item(&query),
        "remove_items" => api_remove_items(&query),
        _ => Err("unknown method".to_string()),
    };

    match result {
        Ok(payload) => qexed_plugin_sdk::response_ptr_len(&PluginApiCallResponse {
            ok: true,
            payload,
            error: String::new(),
        }),
        Err(error) => qexed_plugin_sdk::response_ptr_len(&PluginApiCallResponse {
            ok: false,
            payload: Vec::new(),
            error,
        }),
    }
}

#[derive(Debug, Serialize)]
struct GetItemsResponse {
    items: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize)]
struct CountResponse {
    item: String,
    count: i64,
}

fn api_remove_items(query: &PluginApiCallQuery) -> Result<Vec<u8>, String> {
    /// {player_uuid, items: [{item: "minecraft:stone", count: 64}, ...]}
    #[derive(Deserialize)]
    struct Req {
        player_uuid: String,
        items: Vec<ItemEntry>,
    }
    #[derive(Deserialize)]
    struct ItemEntry {
        item: String,
        count: i64,
    }
    let req: Req = serde_json::from_slice(&query.payload).map_err(|e| format!("parse: {e}"))?;
    let mut backpack = load_backpack(&req.player_uuid);

    let mut removed = BTreeMap::new();
    for entry in &req.items {
        let removed_count = backpack.remove(&entry.item, entry.count);
        if removed_count > 0 {
            removed.insert(entry.item.clone(), removed_count);
        }
    }
    save_backpack(&req.player_uuid, &backpack);

    #[derive(Serialize)]
    struct Resp {
        removed: BTreeMap<String, i64>,
    }
    serde_json::to_vec(&Resp { removed }).map_err(|e| format!("encode: {e}"))
}

fn api_get_items(query: &PluginApiCallQuery) -> Result<Vec<u8>, String> {
    let uuid: String = serde_json::from_slice(&query.payload).map_err(|e| format!("parse: {e}"))?;
    let backpack = load_backpack(&uuid);
    let response = GetItemsResponse {
        items: backpack.items,
    };
    serde_json::to_vec(&response).map_err(|e| format!("encode: {e}"))
}

fn api_get_item_count(query: &PluginApiCallQuery) -> Result<Vec<u8>, String> {
    #[derive(Deserialize)]
    struct Req {
        player_uuid: String,
        item: String,
    }
    let req: Req = serde_json::from_slice(&query.payload).map_err(|e| format!("parse: {e}"))?;
    let backpack = load_backpack(&req.player_uuid);
    let count = backpack.count(&req.item);
    let response = CountResponse {
        item: req.item,
        count,
    };
    serde_json::to_vec(&response).map_err(|e| format!("encode: {e}"))
}

fn api_add_item(query: &PluginApiCallQuery) -> Result<Vec<u8>, String> {
    #[derive(Deserialize)]
    struct Req {
        player_uuid: String,
        item: String,
        count: i64,
    }
    let req: Req = serde_json::from_slice(&query.payload).map_err(|e| format!("parse: {e}"))?;
    let config = load_config();
    let mut backpack = load_backpack(&req.player_uuid);
    let capacity = config.item_capacity(&req.item);
    let _ = backpack.add(&req.item, req.count, capacity);
    save_backpack(&req.player_uuid, &backpack);
    let new_count = backpack.count(&req.item);
    serde_json::to_vec(&CountResponse {
        item: req.item,
        count: new_count,
    })
    .map_err(|e| format!("encode: {e}"))
}

fn api_remove_item(query: &PluginApiCallQuery) -> Result<Vec<u8>, String> {
    #[derive(Deserialize)]
    struct Req {
        player_uuid: String,
        item: String,
        count: i64,
    }
    let req: Req = serde_json::from_slice(&query.payload).map_err(|e| format!("parse: {e}"))?;
    let mut backpack = load_backpack(&req.player_uuid);
    backpack.remove(&req.item, req.count);
    save_backpack(&req.player_uuid, &backpack);
    let new_count = backpack.count(&req.item);
    serde_json::to_vec(&CountResponse {
        item: req.item,
        count: new_count,
    })
    .map_err(|e| format!("encode: {e}"))
}

fn api_error(msg: &str) -> PluginApiCallResponse {
    PluginApiCallResponse {
        ok: false,
        payload: Vec::new(),
        error: msg.to_string(),
    }
}

/// --- Storage helpers -------------------------------------------------------

fn backpack_key(uuid: &str) -> String {
    format!("backpack/{uuid}")
}

fn cooldown_key(uuid: &str) -> String {
    format!("pickup_msg_cooldown/{uuid}")
}

fn load_backpack(uuid: &str) -> PlayerBackpack {
    storage_get_typed::<PlayerBackpack>(&backpack_key(uuid)).unwrap_or_default()
}

fn save_backpack(uuid: &str, backpack: &PlayerBackpack) {
    let _ = storage_set_typed(&backpack_key(uuid), backpack);
}

/// --- Config ----------------------------------------------------------------

fn load_config() -> BackpackConfig {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<BackpackConfig>(&content).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> BackpackConfig {
    toml::from_str(DEFAULT_CONFIG).expect("default backpack config is valid")
}

fn format_name(item_name: &str) -> String {
    item_name
        .strip_prefix("minecraft:")
        .unwrap_or(item_name)
        .replace('_', " ")
}

/// --- Defaults --------------------------------------------------------------

fn default_enable() -> bool {
    true
}

fn default_mode() -> String {
    "disabled".to_string()
}

fn default_infinite_capacity() -> i64 {
    -1
}

fn default_pickup_message() -> String {
    "+{count}x {item} → 背包 ({total})".to_string()
}

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
item = "minecraft:coal_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:raw_copper"
capacity = 10000
sellable = true

[[items]]
item = "minecraft:copper_ingot"
capacity = -1
sellable = true

[[items]]
item = "minecraft:copper_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:raw_iron"
capacity = 10000
sellable = true

[[items]]
item = "minecraft:iron_ingot"
capacity = -1
sellable = true

[[items]]
item = "minecraft:iron_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:raw_gold"
capacity = 10000
sellable = true

[[items]]
item = "minecraft:gold_ingot"
capacity = -1
sellable = true

[[items]]
item = "minecraft:gold_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:redstone"
capacity = -1
sellable = true

[[items]]
item = "minecraft:redstone_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:lapis_lazuli"
capacity = -1
sellable = true

[[items]]
item = "minecraft:lapis_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:diamond"
capacity = 5000
sellable = true

[[items]]
item = "minecraft:diamond_ore"
capacity = -1
sellable = true

[[items]]
item = "minecraft:emerald"
capacity = 5000
sellable = true

[[items]]
item = "minecraft:emerald_ore"
capacity = -1
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
