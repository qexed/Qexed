use qexed_plugin_sdk::PlayerPayload;

qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    50
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        qexed_plugin_sdk::log("player join event decode failed");
        return;
    };
    qexed_plugin_sdk::log(&format!(
        "player joined: username={}, uuid={}, entity_id={}",
        payload.username, payload.uuid, payload.entity_id
    ));
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(ptr: i32, len: i32) {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlayerPayload>(ptr, len) })
    else {
        qexed_plugin_sdk::log("player leave event decode failed");
        return;
    };
    qexed_plugin_sdk::log(&format!(
        "player left: username={}, uuid={}",
        payload.username, payload.uuid
    ));
}
