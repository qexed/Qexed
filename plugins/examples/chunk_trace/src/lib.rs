use qexed_plugin_sdk::ChunkPayload;

qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    -10
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_chunk_load(ptr: i32, len: i32) {
    log_chunk_event("chunk_load", ptr, len);
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_chunk_unload(ptr: i32, len: i32) {
    log_chunk_event("chunk_unload", ptr, len);
}

fn log_chunk_event(event: &str, ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<ChunkPayload>(ptr, len) })
    else {
        qexed_plugin_sdk::log(&format!("{event}: decode failed"));
        return;
    };
    qexed_plugin_sdk::log(&format!(
        "{event}: dimension={}, chunk=({}, {})",
        payload.dimension, payload.chunk_x, payload.chunk_z
    ));
}
