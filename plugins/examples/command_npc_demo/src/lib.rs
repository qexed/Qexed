use qexed_plugin_sdk::{
    NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert, PlaceholderQuery,
    PlaceholderReplacement, PlaceholderResponse, PlayerAction, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, ProxyConnectResultPayload,
};

qexed_plugin_sdk::qexed_plugin_memory!();

const NPC_KEY: &str = "plugin_hub_npc";
const TARGET_SERVER: &str = "lobby-1";

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
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: "hub".to_string(),
        description_key: "commands.trigger.simple.success".to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PluginCommandQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    if payload.command != "hub" {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    if payload.argument.eq_ignore_ascii_case("transfer") {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions: vec![
                PlayerAction::SystemMessage {
                    text: format!("Connecting to {TARGET_SERVER}..."),
                    translate: String::new(),
                    with: Vec::new(),
                    overlay: false,
                },
                PlayerAction::ProxyConnect {
                    server: TARGET_SERVER.to_string(),
                    message: String::new(),
                },
            ],
        });
    }

    qexed_plugin_sdk::response_ptr_len(&hub_teleport_response("Teleporting to hub marker..."))
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_mutations(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&NpcMutationResponse {
        operations: vec![NpcMutationOp::Upsert {
            npc: NpcUpsert {
                key: NPC_KEY.to_string(),
                dimension: "minecraft:overworld".to_string(),
                x: 0.5,
                y: -53.0,
                z: 2.5,
                yaw: 180.0,
                pitch: 0.0,
                name: "Hub NPC".to_string(),
                display_name: "狗策划".to_string(),
                skin_textures: String::new(),
                skin_signature: String::new(),
            },
        }],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_npc_interact(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<NpcInteractPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    qexed_plugin_sdk::log(&format!(
        "npc_interact event: player={}, language={}, dimension={}, key={}, action={}",
        payload.player.username,
        payload.player.language,
        payload.player.dimension,
        payload.entity.key,
        payload.action
    ));

    if payload.entity.key != NPC_KEY {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::SystemMessage {
                text: "Hub NPC clicked. Opening menu...".to_string(),
                translate: String::new(),
                with: Vec::new(),
                overlay: false,
            },
            PlayerAction::OpenMenu {
                menu: "main".to_string(),
            },
        ],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_placeholders(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<PlaceholderQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse::default());
    };
    let language = payload
        .player
        .as_ref()
        .map(|player| player.language.clone())
        .unwrap_or_else(|| "unknown".to_string());
    qexed_plugin_sdk::response_ptr_len(&PlaceholderResponse {
        replacements: vec![
            PlaceholderReplacement {
                key: "demo_language".to_string(),
                value: language,
            },
            PlaceholderReplacement {
                key: "demo_target_server".to_string(),
                value: TARGET_SERVER.to_string(),
            },
        ],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_proxy_connect_result(ptr: i32, len: i32) {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<ProxyConnectResultPayload>(ptr, len) })
    else {
        qexed_plugin_sdk::log("proxy_connect_result decode failed");
        return;
    };

    qexed_plugin_sdk::log(&format!(
        "proxy_connect_result: player={}, target={}, current={}, protocol={}, code={}, status={}, success={}, message={}",
        payload.player.username,
        payload.target_server,
        payload.current_server,
        payload.proxy_protocol,
        payload.status_code,
        payload.status,
        payload.success,
        payload.message
    ));
}

fn hub_teleport_response(message: &str) -> PluginCommandResponse {
    PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::SystemMessage {
                text: message.to_string(),
                translate: String::new(),
                with: Vec::new(),
                overlay: false,
            },
            PlayerAction::Teleport {
                dimension: String::new(),
                x: 0.5,
                y: -53.0,
                z: 0.5,
                yaw: Some(180.0),
                pitch: Some(0.0),
            },
        ],
    }
}
