//! lobby_services：v4 移植 —— 大厅签到/充值/公告。
//!
//! v4：config_reload / player_join / npc_mutations / npc_interact 导出 + manifest。
//! v6：NpcMutationsEvent（Mutex 回写）+ NpcInteractEvent + PlayerJoinEvent + ConfigReloadEvent。
//! 生命周期：enable 时载配置、注册货币（v4 init 语义）。

use qexed_plugin_api::payload::{
    NpcMutationOp, NpcUpsert, PlayerAction as Pa,
};
use qexed_plugin_api::{
    ConfigReloadEvent, LogLevel, NpcInteractEvent, NpcMutationsEvent, PlayerJoinEvent,
};
use qexed_plugin_sdk::{InitFuture, PluginContext};
use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = "config.toml";
const SIGN_IN_NPC_KEY: &str = "lobby:sign_in";
const RECHARGE_NPC_KEY: &str = "lobby:recharge";
const SIGN_IN_EVENT: &str = "lobby_sign_in";
const RECHARGE_EVENT: &str = "lobby_recharge";
const SIGN_IN_PREFIX: &str = "lobby/sign_in/";
const DAY_MS: i64 = 86_400_000;

qexed_plugin_sdk::declare_simple_plugin!("qexed.lobby_services", "Lobby Services", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        let sdk = ctx.sdk();
        let _ = sdk.config_load_or_create(DEFAULT_CONFIG);
        let config = load_config(&sdk);
        register_currency(&sdk, &config);
        ctx.log(LogLevel::Info, "lobby_services initialized");

        // 配置热重载：仅本插件配置路径时重读并重注册货币
        ctx.register::<ConfigReloadEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let path = ev.payload.path.replace('\\', "/");
                if !(path.ends_with("lobby_services/config.toml") || path.ends_with(CONFIG_PATH)) {
                    return;
                }
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                register_currency(&sdk, &config);
                api.log(LogLevel::Info, "lobby_services config reloaded");
            })
        });

        ctx.register::<PlayerJoinEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if config.enable && config.announcement.enable {
                    let text = config
                        .announcement
                        .message
                        .replace("{player}", &ev.player.username);
                    sdk.perform(&ev.player, message(text, config.announcement.overlay));
                }
            })
        });

        // NPC 群体维护（宿主 npc_mutations 收集接线后生效）
        ctx.register::<NpcMutationsEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
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
                    operations.push(NpcMutationOp::Remove { key: SIGN_IN_NPC_KEY.to_string() });
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
                    operations.push(NpcMutationOp::Remove { key: RECHARGE_NPC_KEY.to_string() });
                }
                ev.response.lock().unwrap().operations = operations;
            })
        });

        ctx.register::<NpcInteractEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let sdk = qexed_plugin_sdk::Sdk::new(api, plugin_id);
                let config = load_config(&sdk);
                if !config.enable {
                    return;
                }
                let p = &ev.payload;
                let player = qexed_plugin_api::PlayerInfo {
                    uuid: p.player.uuid.clone(),
                    username: p.player.username.clone(),
                    entity_id: p.player.entity_id,
                    dimension: p.player.dimension.clone(),
                };
                let reply = |text: String| {
                    sdk.perform(&player, message(text, false));
                };
                if p.entity.key == SIGN_IN_NPC_KEY && p.configured_event == SIGN_IN_EVENT {
                    reply(handle_sign_in(&sdk, &config, &p.player.uuid));
                } else if p.entity.key == RECHARGE_NPC_KEY && p.configured_event == RECHARGE_EVENT {
                    reply(config.recharge.message.replace("{player}", &p.player.username));
                }
            })
        });
    })
});

/// 全 Default 形状的配置（serde 默认值驱动），default_config 用它种子化。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ConfigTomlDefault {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SignInRecord {
    #[serde(default)]
    last_day: i64,
    #[serde(default)]
    total_days: i64,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    #[serde(default)]
    npc: NpcConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RechargeConfig {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_recharge_message")]
    message: String,
    #[serde(default)]
    npc: NpcConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AnnouncementConfig {
    #[serde(default)]
    enable: bool,
    #[serde(default = "default_announcement_message")]
    message: String,
    #[serde(default)]
    overlay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NpcConfig {
    #[serde(default = "default_dimension")]
    dimension: String,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    #[serde(default)]
    z: f64,
    #[serde(default)]
    yaw: f32,
    #[serde(default)]
    pitch: f32,
    #[serde(default)]
    entity_type: String,
    #[serde(default)]
    display_name: String,
}
impl Default for NpcConfig {
    fn default() -> Self {
        Self {
            dimension: default_dimension(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            entity_type: "minecraft:villager".to_string(),
            display_name: String::new(),
        }
    }
}

fn default_true() -> bool { true }
fn default_currency_id() -> String { "qexed:coin".to_string() }
fn default_currency_name() -> String { "Coin".to_string() }
fn default_currency_symbol() -> String { "Q".to_string() }
fn default_currency_fractional_digits() -> i32 { 2 }
fn default_sign_in_reward_amount() -> i64 { 1000 }
fn default_sign_in_day_offset_hours() -> i32 { 8 }
fn default_sign_in_success_message() -> String {
    "签到成功，获得 {reward}，当前余额 {balance}，累计签到 {days} 天。".to_string()
}
fn default_sign_in_already_message() -> String { "今天已经签到过了，明天再来吧。".to_string() }
fn default_sign_in_failure_message() -> String { "签到失败，货币系统暂时不可用。".to_string() }
fn default_recharge_message() -> String { "本服暂未支持充值。".to_string() }
fn default_announcement_message() -> String { "欢迎 {player} 来到大厅！".to_string() }
fn default_dimension() -> String { "minecraft:overworld".to_string() }

fn handle_sign_in(sdk: &qexed_plugin_sdk::Sdk, config: &Config, player_uuid: &str) -> String {
    if !config.sign_in.enable {
        return config.sign_in.failure_message.clone();
    }
    let today = current_day(config);
    let key = format!("{SIGN_IN_PREFIX}{player_uuid}");
    let mut record: SignInRecord = sdk.storage_get_json(&key).unwrap_or_default();
    if record.last_day == today {
        return config.sign_in.already_message.clone();
    }
    let reward = config.sign_in.reward_amount.max(0);
    let Some(_balance) = sdk.economy_deposit(player_uuid, &config.currency.id, reward) else {
        return config.sign_in.failure_message.clone();
    };
    record.last_day = today;
    record.total_days = record.total_days.saturating_add(1);
    let _ = sdk.storage_set_json(&key, &record);
    config
        .sign_in
        .success_message
        .replace("{reward}", &format_money(reward, &config.currency))
        .replace("{balance}", &format_money(_balance, &config.currency))
        .replace("{days}", &record.total_days.to_string())
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

fn register_currency(sdk: &qexed_plugin_sdk::Sdk, config: &Config) {
    let _ = sdk.economy_register_currency(
        &config.currency.id,
        &config.currency.name,
        &config.currency.symbol,
        config.currency.fractional_digits,
    );
}

fn message(text: impl Into<String>, overlay: bool) -> Pa {
    Pa::SystemMessage { text: text.into(), translate: String::new(), with: Vec::new(), overlay }
}

fn current_day(config: &Config) -> i64 {
    let offset_ms = i64::from(config.sign_in.day_offset_hours).clamp(-23, 23) * 3_600_000;
    (now_millis().saturating_add(offset_ms)).max(0) / DAY_MS
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn format_money(amount: i64, currency: &CurrencyConfig) -> String {
    let digits = currency.fractional_digits.clamp(0, 8) as u32;
    if digits == 0 {
        return format!("{}{}", amount, currency.symbol);
    }
    let scale = 10_i64.pow(digits);
    let sign = if amount < 0 { "-" } else { "" };
    let amount = amount.abs();
    format!("{}{}.{:0width$}{}", sign, amount / scale, amount % scale, currency.symbol, width = digits as usize)
}

fn load_config(sdk: &qexed_plugin_sdk::Sdk) -> Config {
    sdk.read_config()
        .ok()
        .flatten()
        .and_then(|content| {
            content
                .parse::<toml_edit::DocumentMut>()
                .ok()
                .and_then(|doc| qexed_toml::from_document::<Config>(doc).ok())
        })
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    let json = serde_json::to_string(&ConfigTomlDefault::default()).expect("serialize defaults");
    serde_json::from_str(&json).expect("default lobby services config is valid")
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
message = "欢迎 {player} 来到大厅！"
overlay = false
"#;
