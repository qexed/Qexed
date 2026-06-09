use qexed_plugin_sdk::{
    PlayerAction, PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse,
    PluginDependency, PluginManifest,
};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.demo.consumer".to_string(),
    version: "0.1.0".to_string(),
    depends: vec![PluginDependency {
        id: "qexed.demo.provider".to_string(),
        version: "0.1.0".to_string(),
    }],
    optional_depends: Vec::new(),
    services: Vec::new(),
});

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: "apitest".to_string(),
        description_key: "qexed.plugin.api_test.description".to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    let Some(query) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PluginCommandQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };
    if query.command != "apitest" {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let storage = qexed_plugin_sdk::economy_storage("qexed:coin")
        .unwrap_or_else(|| "unknown".to_string());
    let exists = qexed_plugin_sdk::plugin_service_exists("qexed.demo.echo");
    let response = qexed_plugin_sdk::plugin_call("qexed.demo.echo", "echo", b"ok")
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| "call_failed".to_string());

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::SystemMessage {
            text: format!("api={response};exists={exists};storage={storage}"),
            translate: String::new(),
            with: Vec::new(),
            overlay: false,
        }],
    })
}
