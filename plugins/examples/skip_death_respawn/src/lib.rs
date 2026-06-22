use qexed_plugin_sdk::{
    ConfigReloadPayload, PlayerDeathQuery, PlayerDeathResponse, PluginManifest,
    config_load_or_create, config_read_to_string,
};
use serde::{Deserialize, Serialize};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.skip_death_respawn".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    load_after: Vec::new(),
    services: Vec::new(),
});

const CONFIG_PATH: &str = "config.toml";

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    270
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = config_load_or_create(CONFIG_PATH, DEFAULT_CONFIG);
    qexed_plugin_sdk::log("skip_death_respawn initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        return;
    };
    let path = payload.path.replace('\\', "/");
    if path.ends_with("skip_death_respawn/config.toml") || path.ends_with(CONFIG_PATH) {
        let _ = load_config();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_death(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerDeathQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlayerDeathResponse::default());
    };
    let config = load_config();
    if !config.enable {
        return qexed_plugin_sdk::response_ptr_len(&PlayerDeathResponse::default());
    }

    let message = if config.message_enable {
        config
            .message
            .replace("{player}", &payload.player.username)
            .replace("{cause}", &payload.cause)
    } else {
        String::new()
    };

    qexed_plugin_sdk::response_ptr_len(&PlayerDeathResponse {
        cancel: true,
        message,
        overlay: config.message_overlay,
    })
}

fn load_config() -> Config {
    config_read_to_string(CONFIG_PATH)
        .and_then(|content| toml::from_str::<Config>(&content).ok())
        .unwrap_or_else(default_config)
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("default skip death respawn config is valid")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default = "default_true")]
    enable: bool,
    #[serde(default = "default_true")]
    message_enable: bool,
    #[serde(default = "default_message")]
    message: String,
    #[serde(default)]
    message_overlay: bool,
}

fn default_true() -> bool {
    true
}

fn default_message() -> String {
    "已跳过死亡，返回出生点。".to_string()
}

const DEFAULT_CONFIG: &str = r#"# 跳过死亡返回出生点配置
enable = true

message_enable = true
message = "已跳过死亡，返回出生点。"
message_overlay = false
"#;
