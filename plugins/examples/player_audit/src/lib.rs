qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    50
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    let username = qexed_plugin_sdk::json_string_field(payload, "username").unwrap_or("unknown");
    let uuid = qexed_plugin_sdk::json_string_field(payload, "uuid").unwrap_or("unknown");
    let entity_id = qexed_plugin_sdk::json_i32_field(payload, "entity_id").unwrap_or_default();
    qexed_plugin_sdk::log(&format!(
        "玩家进入服务器: username={username}, uuid={uuid}, entity_id={entity_id}"
    ));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    let username = qexed_plugin_sdk::json_string_field(payload, "username").unwrap_or("unknown");
    let uuid = qexed_plugin_sdk::json_string_field(payload, "uuid").unwrap_or("unknown");
    qexed_plugin_sdk::log(&format!("玩家离开服务器: username={username}, uuid={uuid}"));
}
