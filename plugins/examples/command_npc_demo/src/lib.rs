use qexed_plugin_sdk::{
    CustomEntityDefinition, CustomEntityRegistryResponse, EntityAiOperation, EntityAiTickQuery,
    EntityAiTickResponse, NpcInteractPayload, NpcMutationOp, NpcMutationResponse, NpcUpsert,
    PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse, PlayerAction,
    PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse, ProxyConnectResultPayload,
    config_load_or_create, config_read_to_string, economy_currency_info,
    economy_register_currency,
};

qexed_plugin_sdk::qexed_plugin_memory!();

const NPC_KEY: &str = "plugin_hub_npc";
const DEFAULT_TARGET_SERVER: &str = "survival_1";

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    200
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("command_npc_demo initialized");
    let _config = config_load_or_create("config.toml", "target_server = \"survival_1\"\n");
    let _ = economy_register_currency("qexed:coin", "Coin", "Q", 2);
    if let Some(currency) = economy_currency_info("qexed:coin") {
        qexed_plugin_sdk::log(&format!(
            "economy currency registered: id={}, name={}, symbol={}",
            currency.id, currency.name, currency.symbol
        ));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&PluginCommandDefinition {
        name: "hub".to_string(),
        description_key: "commands.trigger.simple.success".to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_custom_entities(_ptr: i32, _len: i32) -> i64 {
    qexed_plugin_sdk::response_ptr_len(&CustomEntityRegistryResponse {
        entities: vec![CustomEntityDefinition {
            id: "demo:patrol_guard".to_string(),
            entity_type: "minecraft:villager".to_string(),
            display_name: "{\"text\":\"Patrol Guard\",\"color\":\"gold\"}".to_string(),
            ai: "plugin:demo_patrol".to_string(),
            ai_params: [
                ("iq".to_string(), serde_json::json!(80)),
                ("patrol_min_x".to_string(), serde_json::json!(4.0)),
                ("patrol_max_x".to_string(), serde_json::json!(12.0)),
            ]
            .into_iter()
            .collect(),
        }],
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_entity_ai_tick(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<EntityAiTickQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&EntityAiTickResponse::default());
    };

    if payload.entity.custom_type != "demo:patrol_guard"
        || payload.entity.ai != "plugin:demo_patrol"
    {
        return qexed_plugin_sdk::response_ptr_len(&EntityAiTickResponse::default());
    }

    let patrol_min_x = payload
        .entity
        .ai_params
        .get("patrol_min_x")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(4.0);
    let patrol_max_x = payload
        .entity
        .ai_params
        .get("patrol_max_x")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(12.0);
    let speed = payload
        .entity
        .ai_params
        .get("speed")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.05)
        .clamp(0.0, 0.5);
    if let Some(iq) = payload
        .entity
        .ai_params
        .get("iq")
        .and_then(serde_json::Value::as_i64)
    {
        qexed_plugin_sdk::log(&format!(
            "demo patrol ai params: key={}, iq={iq}",
            payload.entity.key
        ));
    }

    let direction = if payload.entity.position.x > patrol_max_x {
        -1.0
    } else if payload.entity.position.x < patrol_min_x {
        1.0
    } else if payload.entity.entity_id % 2 == 0 {
        1.0
    } else {
        -1.0
    };

    qexed_plugin_sdk::response_ptr_len(&EntityAiTickResponse {
        operations: vec![EntityAiOperation::MoveDelta {
            x: speed * direction,
            y: 0.0,
            z: 0.0,
            yaw: Some(if direction > 0.0 { -90.0 } else { 90.0 }),
            pitch: Some(0.0),
        }],
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
        let target_server = target_server();
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
            handled: true,
            actions: vec![
                PlayerAction::SystemMessage {
                    text: format!("Connecting to {target_server}..."),
                    translate: String::new(),
                    with: Vec::new(),
                    overlay: false,
                },
                PlayerAction::ProxyConnect {
                    server: target_server,
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
                look_at_players: true,
                main_hand_event: "interact".to_string(),
                off_hand_event: "interact_off_hand".to_string(),
                attack_event: "attack".to_string(),
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

    qexed_plugin_sdk::response_ptr_len(&proxy_connect_response("Hub NPC clicked. Connecting..."))
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
                value: target_server(),
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

fn proxy_connect_response(message: &str) -> PluginCommandResponse {
    let target_server = target_server();
    PluginCommandResponse {
        handled: true,
        actions: vec![
            PlayerAction::SystemMessage {
                text: message.to_string(),
                translate: String::new(),
                with: Vec::new(),
                overlay: false,
            },
            PlayerAction::ProxyConnect {
                server: target_server,
                message: String::new(),
            },
        ],
    }
}

fn target_server() -> String {
    config_read_to_string("config.toml")
        .and_then(|config| string_setting(&config, "target_server"))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TARGET_SERVER.to_string())
}

fn string_setting(config: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    config.lines().find_map(|line| {
        let line = line.trim();
        let value = line.strip_prefix(&prefix)?.trim();
        let value = value.strip_prefix('"')?.strip_suffix('"')?;
        Some(value.to_string())
    })
}
