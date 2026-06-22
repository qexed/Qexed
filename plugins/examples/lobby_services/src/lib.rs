use qexed_plugin_sdk::{
    ConfigReloadPayload, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlayerAction, PlayerPayload, PluginCommandResponse, PluginManifest, config_load_or_create,
    config_read_to_string, economy_deposit, economy_register_currency, storage_get_typed,
    storage_set_typed, time_millis,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.lobby_services".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    load_after: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";
const SIGN_IN_NPC_KEY: &str = "lobby:sign_in";
const RECHARGE_NPC_KEY: &str = "lobby:recharge";
const SIGN_IN_EVENT: &str = "lobby_sign_in";
const RECHARGE_EVENT: &str = "lobby_recharge";
const SIGN_IN_PREFIX: &str = "lobby/sign_in/";
const DAY_MS: i64 = 86_400_000;

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    260
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    register_currency(&load_config());
    qexed_plugin_sdk::log("lobby_services initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("lobby_services/config.toml") || path.ends_with(CONFIG_PATH) {
        register_currency(&load_config());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) -> i64 {
    let Some(player) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&Vec::<PlayerAction>::new());
    };
    let config = load_config();
    let actions = if config.enable && config.announcement.enable {
        vec![message(
            config
                .announcement
                .message
                .replace("{player}", &player.username),
            config.announcement.overlay,
        )]
    } else {
        Vec::new()
    };
    qexed_plugin_sdk::response_ptr_len(&actions)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_mutations(_ptr: i32, _len: i32) -> i64 {
    let config = load_config();

    let mut operations = Vec::new();
    if config.enable && config.sign_in.enable {
        operations.push(NpcMutationOp::Upsert {
            npc: npc_upsert(
                SIGN_IN_NPC_KEY,
                SIGN_IN_EVENT,
                "Lobby Sign In",
                &config.sign_in.npc,
            ),
        });
    } else {
        operations.push(remove_npc(SIGN_IN_NPC_KEY));
    }
    if config.enable && config.recharge.enable {
        operations.push(NpcMutationOp::Upsert {
            npc: npc_upsert(
                RECHARGE_NPC_KEY,
                RECHARGE_EVENT,
                "Lobby Recharge",
                &config.recharge.npc,
            ),
        });
    } else {
        operations.push(remove_npc(RECHARGE_NPC_KEY));
    }

    qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse { operations })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<NpcInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let response = match payload.entity.key.as_str() {
        SIGN_IN_NPC_KEY if payload.configured_event == SIGN_IN_EVENT => {
            handle_sign_in(&config, &payload.player.uuid)
        }
        RECHARGE_NPC_KEY if payload.configured_event == RECHARGE_EVENT => {
            handled_message(config.recharge.message.clone(), false)
        }
        _ => PluginCommandResponse::default(),
    };
    qexed_plugin_sdk::response_ptr_len(&response)
}

fn handle_sign_in(config: &Config, player_uuid: &str) -> PluginCommandResponse {
    if !config.sign_in.enable {
        return PluginCommandResponse::default();
    }

    let today = current_day(config);
    let key = sign_in_key(player_uuid);
    let mut record = storage_get_typed::<SignInRecord>(&key).unwrap_or_default();
    if record.last_day == today {
        return handled_message(config.sign_in.already_message.clone(), false);
    }

    register_currency(config);
    let reward = config.sign_in.reward_amount.max(0);
    let Some(balance) = economy_deposit(player_uuid, &config.currency.id, reward) else {
        return handled_message(config.sign_in.failure_message.clone(), false);
    };

    record.last_day = today;
    record.total_days = record.total_days.saturating_add(1);
    let _ = storage_set_typed(&key, &record);

    handled_message(
        config
            .sign_in
            .success_message
            .replace("{reward}", &format_money(reward, &config.currency))
            .replace("{balance}", &format_money(balance, &config.currency))
            .replace("{days}", &record.total_days.to_string()),
        false,
    )
}

fn npc_upsert(key: &str, event: &str, name: &str, config: &NpcConfig) -> NpcUpsert {
    NpcUpsert {
        key: key.to_string(),
        dimension: config.dimension.clone(),
        x: config.x,
        y: config.y,
        z: config.z,
        yaw: config.yaw,
        pitch: config.pitch,
        name: name.to_string(),
        display_name: config.display_name.clone(),
        entity_type: config.entity_type.clone(),
        skin_textures: String::new(),
        skin_signature: String::new(),
        look_at_players: true,
        main_hand_event: event.to_string(),
        off_hand_event: event.to_string(),
        attack_event: event.to_string(),
    }
}

fn register_currency(config: &Config) {
    let _ = economy_register_currency(
        &config.currency.id,
        &config.currency.name,
        &config.currency.symbol,
        config.currency.fractional_digits,
    );
}

fn message(text: impl Into<String>, overlay: bool) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay,
    }
}

fn handled_message(text: impl Into<String>, overlay: bool) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![message(text, overlay)],
    }
}

fn sign_in_key(player_uuid: &str) -> String {
    format!("{SIGN_IN_PREFIX}{player_uuid}")
}

fn current_day(config: &Config) -> i64 {
    let offset_ms = i64::from(config.sign_in.day_offset_hours).clamp(-23, 23) * 3_600_000;
    time_millis().saturating_add(offset_ms).max(0) / DAY_MS
}

fn format_money(amount: i64, currency: &CurrencyConfig) -> String {
    let digits = currency.fractional_digits.clamp(0, 8) as u32;
    if digits == 0 {
        return format!("{}{}", amount, currency.symbol);
    }

    let scale = 10_i64.pow(digits);
    let sign = if amount < 0 { "-" } else { "" };
    let amount = amount.abs();
    format!(
        "{}{}.{:0width$}{}",
        sign,
        amount / scale,
        amount % scale,
        currency.symbol,
        width = digits as usize
    )
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<Config>(&content).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default lobby services config is valid")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default)]
    currency: CurrencyConfig,
    #[serde(default)]
    sign_in: SignInConfig,
    #[serde(default)]
    recharge: RechargeConfig,
    #[serde(default)]
    announcement: AnnouncementConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurrencyConfig {
    #[serde(default = "default_currency_id")]
    id: String,
    #[serde(default = "default_currency_name")]
    name: String,
    #[serde(default = "default_currency_symbol")]
    symbol: String,
    #[serde(default = "default_currency_fractional_digits")]
    fractional_digits: i32,
}

impl Default for CurrencyConfig {
    fn default() -> Self {
        Self {
            id: default_currency_id(),
            name: default_currency_name(),
            symbol: default_currency_symbol(),
            fractional_digits: default_currency_fractional_digits(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignInConfig {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_sign_in_reward_amount")]
    reward_amount: i64,
    #[serde(default = "default_sign_in_day_offset_hours")]
    day_offset_hours: i32,
    #[serde(default = "default_sign_in_success_message")]
    success_message: String,
    #[serde(default = "default_sign_in_already_message")]
    already_message: String,
    #[serde(default = "default_sign_in_failure_message")]
    failure_message: String,
    #[serde(default = "default_sign_in_npc")]
    npc: NpcConfig,
}

impl Default for SignInConfig {
    fn default() -> Self {
        Self {
            enable: true,
            reward_amount: default_sign_in_reward_amount(),
            day_offset_hours: default_sign_in_day_offset_hours(),
            success_message: default_sign_in_success_message(),
            already_message: default_sign_in_already_message(),
            failure_message: default_sign_in_failure_message(),
            npc: default_sign_in_npc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RechargeConfig {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_recharge_message")]
    message: String,
    #[serde(default = "default_recharge_npc")]
    npc: NpcConfig,
}

impl Default for RechargeConfig {
    fn default() -> Self {
        Self {
            enable: true,
            message: default_recharge_message(),
            npc: default_recharge_npc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnnouncementConfig {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_announcement_message")]
    message: String,
    #[serde(default)]
    overlay: bool,
}

impl Default for AnnouncementConfig {
    fn default() -> Self {
        Self {
            enable: true,
            message: default_announcement_message(),
            overlay: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NpcConfig {
    #[serde(default = "default_dimension")]
    dimension: String,
    x: f64,
    y: f64,
    z: f64,
    #[serde(default)]
    yaw: f32,
    #[serde(default)]
    pitch: f32,
    #[serde(default = "default_npc_entity_type")]
    entity_type: String,
    #[serde(default)]
    display_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SignInRecord {
    last_day: i64,
    total_days: u64,
}

fn default_true() -> bool {
    true
}

fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_currency_id() -> String {
    "qexed:coin".to_string()
}

fn default_currency_name() -> String {
    "Coin".to_string()
}

fn default_currency_symbol() -> String {
    "Q".to_string()
}

fn default_currency_fractional_digits() -> i32 {
    2
}

fn default_sign_in_reward_amount() -> i64 {
    1_000
}

fn default_sign_in_day_offset_hours() -> i32 {
    8
}

fn default_sign_in_success_message() -> String {
    "签到成功，获得 {reward}，当前余额 {balance}，累计签到 {days} 天。".to_string()
}

fn default_sign_in_already_message() -> String {
    "今天已经签到过了，明天再来吧。".to_string()
}

fn default_sign_in_failure_message() -> String {
    "签到失败，货币系统暂时不可用。".to_string()
}

fn default_recharge_message() -> String {
    "本服暂未支持充值。".to_string()
}

fn default_announcement_message() -> String {
    "欢迎来到服务器大厅，{player}！".to_string()
}

fn default_npc_entity_type() -> String {
    "minecraft:villager".to_string()
}

fn default_sign_in_npc() -> NpcConfig {
    NpcConfig {
        dimension: default_dimension(),
        x: 253.0,
        y: 30.0,
        z: 151.0,
        yaw: 180.0,
        pitch: 0.0,
        entity_type: default_npc_entity_type(),
        display_name: "{\"text\":\"每日签到\",\"color\":\"gold\",\"bold\":true}".to_string(),
    }
}

fn default_recharge_npc() -> NpcConfig {
    NpcConfig {
        dimension: default_dimension(),
        x: 253.0,
        y: 30.0,
        z: 156.0,
        yaw: -100.0,
        pitch: 0.0,
        entity_type: default_npc_entity_type(),
        display_name: "{\"text\":\"充值\",\"color\":\"aqua\",\"bold\":true}".to_string(),
    }
}

fn remove_npc(key: &str) -> NpcMutationOp {
    NpcMutationOp::Remove {
        key: key.to_string(),
    }
}

const DEFAULT_CONFIG: &str = r#"# 大厅服插件配置
enable = true

[currency]
id = "qexed:coin"
name = "Coin"
symbol = "Q"
fractional_digits = 2

[sign_in]
enable = true
# 金额使用货币最小单位。qexed:coin 默认 2 位小数，1000 = 10.00Q。
reward_amount = 1000
# 签到自然日偏移，8 表示按 UTC+8 自然日刷新。
day_offset_hours = 8
success_message = "签到成功，获得 {reward}，当前余额 {balance}，累计签到 {days} 天。"
already_message = "今天已经签到过了，明天再来吧。"
failure_message = "签到失败，货币系统暂时不可用。"

[sign_in.npc]
dimension = "minecraft:overworld"
x = 253.0
y = 30.0
z = 151.0
yaw = 180.0
pitch = 0.0
entity_type = "minecraft:villager"
display_name = "{\"text\":\"每日签到\",\"color\":\"gold\",\"bold\":true}"

[recharge]
enable = true
message = "本服暂未支持充值。"

[recharge.npc]
dimension = "minecraft:overworld"
x = 253.0
y = 30.0
z = 156.0
yaw = -100.0
pitch = 0.0
entity_type = "minecraft:villager"
display_name = "{\"text\":\"充值\",\"color\":\"aqua\",\"bold\":true}"

[announcement]
enable = true
message = "欢迎来到服务器大厅，{player}！"
overlay = false
"#;
