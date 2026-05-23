qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("hello_world 初始化完成");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_config_reload(ptr: i32, len: i32) {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    qexed_plugin_sdk::log(&format!("收到配置加载事件: {payload}"));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_language_change(ptr: i32, len: i32) {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    let language = qexed_plugin_sdk::json_string_field(payload, "language").unwrap_or("unknown");
    qexed_plugin_sdk::log(&format!("当前语言: {language}"));
}
