use qexed_plugin_sdk::{PlayerAction, PlayerPayload, PluginCommandDefinition, decode_payload, log};

qexed_plugin_sdk::qexed_plugin_memory!();

/// Starter items for a new skyblock player.
const STARTER_KIT: &[(&str, i32)] = &[
    ("minecraft:lava_bucket", 1),
    ("minecraft:ice", 2),
    ("minecraft:bone_meal", 8),
    ("minecraft:oak_sapling", 2),
    ("minecraft:dirt", 3),
    ("minecraft:apple", 5),
];

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    log("Skyblock plugin initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_manifest(_ptr: i32, _len: i32) -> i64 {
    use serde::Serialize;
    #[derive(Serialize)]
    struct Manifest {
        id: String,
        version: String,
    }
    qexed_plugin_sdk::response_ptr_len(&Manifest {
        id: "skyblock".to_string(),
        version: "0.1.0".to_string(),
    })
}

/// Player join: give starter kit + welcome message
#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_join(ptr: i32, len: i32) -> i64 {
    let Some(player) = (unsafe { decode_payload::<PlayerPayload>(ptr, len) }) else {
        log("player_join: decode failed");
        return 0;
    };

    let key = format!("skyblock:joined:{}", player.uuid);
    if qexed_plugin_sdk::storage_exists(&key) {
        log(&format!("player {} returned", player.username));
        return 0;
    }

    log(&format!(
        "new skyblock player: {} (uuid={}), giving starter kit",
        player.username, player.uuid
    ));

    // Build actions
    let mut actions: Vec<PlayerAction> = vec![
        PlayerAction::SystemMessage {
            text: format!(
                "§aWelcome to Skyblock, {}! §eStarter items placed in inventory.",
                player.username
            ),
            translate: String::new(),
            with: Vec::new(),
            overlay: false,
        },
        PlayerAction::SystemMessage {
            text: "§7Tip: Place ice to get water, lava + water = cobblestone generator."
                .to_string(),
            translate: String::new(),
            with: Vec::new(),
            overlay: false,
        },
    ];

    for (item, count) in STARTER_KIT {
        actions.push(PlayerAction::GiveItem {
            item: item.to_string(),
            count: *count,
            name: String::new(),
            lore: Vec::new(),
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
        });
    }

    qexed_plugin_sdk::storage_set(&key, b"1");

    qexed_plugin_sdk::response_ptr_len(&actions)
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_leave(_ptr: i32, _len: i32) {
    // No cleanup needed
}

/// Register /skyblock and /island commands
#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_commands(_ptr: i32, _len: i32) -> i64 {
    let cmds = vec![
        PluginCommandDefinition {
            name: "skyblock".to_string(),
            description_key: "Skyblock help".to_string(),
        },
        PluginCommandDefinition {
            name: "island".to_string(),
            description_key: "Island info".to_string(),
        },
    ];
    qexed_plugin_sdk::response_ptr_len(&cmds)
}

/// Handle /skyblock and /island commands
#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_command_execute(ptr: i32, len: i32) -> i64 {
    use qexed_plugin_sdk::PluginCommandResponse;

    let response = PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::SystemMessage {
            text: "§6=== Skyblock ===\n§e/skyblock help §7- Show help\n§e/island §7- Island info"
                .to_string(),
            translate: String::new(),
            with: Vec::new(),
            overlay: false,
        }],
    };
    qexed_plugin_sdk::response_ptr_len(&response)
}
