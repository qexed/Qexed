qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    -10
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_chunk_load(ptr: i32, len: i32) {
    log_chunk_event("区块加载", ptr, len);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_chunk_unload(ptr: i32, len: i32) {
    log_chunk_event("区块卸载", ptr, len);
}

fn log_chunk_event(event: &str, ptr: i32, len: i32) {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    let dimension = qexed_plugin_sdk::json_string_field(payload, "dimension").unwrap_or("unknown");
    let chunk_x = qexed_plugin_sdk::json_i32_field(payload, "chunk_x").unwrap_or_default();
    let chunk_z = qexed_plugin_sdk::json_i32_field(payload, "chunk_z").unwrap_or_default();
    qexed_plugin_sdk::log(&format!(
        "{event}: dimension={dimension}, chunk=({chunk_x}, {chunk_z})"
    ));
}
