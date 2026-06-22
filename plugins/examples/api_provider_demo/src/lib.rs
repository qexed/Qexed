use qexed_plugin_sdk::{
    PluginApiCallQuery, PluginApiCallResponse, PluginManifest, PluginServiceDefinition,
};

qexed_plugin_sdk::qexed_plugin_memory!();
qexed_plugin_sdk::qexed_plugin_manifest!(PluginManifest {
    id: "qexed.demo.provider".to_string(),
    version: "0.1.0".to_string(),
    depends: Vec::new(),
    optional_depends: Vec::new(),
    load_after: Vec::new(),
    services: vec![PluginServiceDefinition {
        id: "qexed.demo.echo".to_string(),
        version: "0.1.0".to_string(),
        methods: vec!["echo".to_string()],
    }],
});

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    200
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("api_provider_demo initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_api_call(ptr: i32, len: i32) -> i64 {
    let Some(query) = (unsafe { qexed_plugin_sdk::decode_payload::<PluginApiCallQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginApiCallResponse {
            ok: false,
            payload: Vec::new(),
            error: "decode failed".to_string(),
        });
    };

    if query.service != "qexed.demo.echo" || query.method != "echo" {
        return qexed_plugin_sdk::response_ptr_len(&PluginApiCallResponse {
            ok: false,
            payload: Vec::new(),
            error: "unknown service method".to_string(),
        });
    }

    let mut response = b"provider:".to_vec();
    response.extend(query.payload);
    qexed_plugin_sdk::response_ptr_len(&PluginApiCallResponse {
        ok: true,
        payload: response,
        error: String::new(),
    })
}
