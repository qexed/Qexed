use qexed_plugin_sdk::{ConfigReloadPayload, LanguagePayload};

qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("hello_world initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ConfigReloadPayload>(ptr, len) })
    else {
        qexed_plugin_sdk::log("config reload event decode failed");
        return;
    };
    qexed_plugin_sdk::log(&format!("config loaded: {}", payload.path));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_language_change(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<LanguagePayload>(ptr, len) })
    else {
        qexed_plugin_sdk::log("language change event decode failed");
        return;
    };
    qexed_plugin_sdk::log(&format!("current language: {}", payload.language));
}
