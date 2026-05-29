use qexed_plugin_sdk::{BlockStepPayload, PlayerAction, PluginCommandResponse};

qexed_plugin_sdk::qexed_plugin_memory!();

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("slime_jump initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_block_step(ptr: i32, len: i32) -> i64 {
    let Some(payload) = (unsafe { qexed_plugin_sdk::decode_payload::<BlockStepPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    if payload.block_name != "minecraft:slime_block" {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let (x, z) = forward_velocity(payload.player_position.yaw, 0.85);
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::Velocity {
            x,
            y: 1.35,
            z,
            additive: false,
        }],
    })
}

fn forward_velocity(yaw: f32, speed: f64) -> (f64, f64) {
    let radians = f64::from(yaw).to_radians();
    (-radians.sin() * speed, radians.cos() * speed)
}
