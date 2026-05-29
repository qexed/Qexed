use std::cell::RefCell;
use std::collections::BTreeMap;

use qexed_plugin_sdk::{PlayerAction, PlayerInputPayload, PluginCommandResponse};

qexed_plugin_sdk::qexed_plugin_memory!();

thread_local! {
    static JUMP_STATE: RefCell<BTreeMap<String, JumpState>> = const { RefCell::new(BTreeMap::new()) };
}

#[derive(Debug, Default, Clone, Copy)]
struct JumpState {
    released_jump_in_air: bool,
    used_double_jump: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    90
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    qexed_plugin_sdk::log("double_jump initialized");
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_player_input(ptr: i32, len: i32) -> i64 {
    let Some(payload) =
        (unsafe { qexed_plugin_sdk::decode_payload::<PlayerInputPayload>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    };

    if payload.position.on_ground {
        JUMP_STATE.with(|states| {
            states
                .borrow_mut()
                .insert(payload.player.uuid.clone(), JumpState::default());
        });
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let jump_pressed = !payload.previous_input.jump && payload.input.jump;
    if !jump_pressed {
        if !payload.input.jump {
            JUMP_STATE.with(|states| {
                states
                    .borrow_mut()
                    .entry(payload.player.uuid.clone())
                    .or_default()
                    .released_jump_in_air = true;
            });
        }
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let should_jump = JUMP_STATE.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.entry(payload.player.uuid.clone()).or_default();
        if state.released_jump_in_air && !state.used_double_jump {
            state.used_double_jump = true;
            true
        } else {
            false
        }
    });

    if !should_jump {
        return qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse::default());
    }

    let (x, z) = forward_velocity(payload.position.yaw, 0.55);
    qexed_plugin_sdk::response_ptr_len(&PluginCommandResponse {
        handled: true,
        actions: vec![PlayerAction::Velocity {
            x,
            y: 0.9,
            z,
            additive: false,
        }],
    })
}

fn forward_velocity(yaw: f32, speed: f64) -> (f64, f64) {
    let radians = f64::from(yaw).to_radians();
    (-radians.sin() * speed, radians.cos() * speed)
}
