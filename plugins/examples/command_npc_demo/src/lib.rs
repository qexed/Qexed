qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    200
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("command_npc_demo initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(
        r#"{"name":"hub","description_key":"commands.trigger.simple.success"}"#,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    let command = qexed_plugin_sdk::json_string_field(payload, "command").unwrap_or("");
    let argument = qexed_plugin_sdk::json_string_field(payload, "argument").unwrap_or("");

    if command != "hub" {
        return qexed_plugin_sdk::response_ptr_len(r#"{"handled":false,"actions":[]}"#);
    }

    if argument.eq_ignore_ascii_case("transfer") {
        return qexed_plugin_sdk::response_ptr_len(
            r#"{"handled":true,"actions":[{"type":"system_message","text":"Connecting to lobby-backend..."},{"type":"transfer","host":"127.0.0.1","port":25566}]}"#,
        );
    }

    qexed_plugin_sdk::response_ptr_len(
        r#"{"handled":true,"actions":[{"type":"system_message","text":"Teleporting to hub marker..."},{"type":"teleport","x":0.5,"y":100.0,"z":0.5,"yaw":180.0,"pitch":0.0}]}"#,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_mutations(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(
        r#"{"operations":[{"op":"upsert","npc":{"key":"plugin_hub_npc","dimension":"minecraft:overworld","x":0.5,"y":99.0,"z":2.5,"yaw":180.0,"pitch":0.0,"name":"Hub NPC","display_name":"Hub NPC"}}]}"#,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_interact(ptr: i32, len: i32) {
    let payload = unsafe { qexed_plugin_sdk::payload_str(ptr, len) }.unwrap_or("{}");
    let player = qexed_plugin_sdk::json_string_field(payload, "username").unwrap_or("unknown");
    let entity = qexed_plugin_sdk::json_string_field(payload, "key").unwrap_or("unknown");
    qexed_plugin_sdk::log(&format!(
        "npc_interact event: player={player}, key={entity}, payload={payload}"
    ));
}
