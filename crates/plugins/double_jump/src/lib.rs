//! double_jump：v4 示例移植 —— 空中再跳一次。
//!
//! v4 在 player_input 事件里返回 Velocity 动作；v6 在 handler 内直接
//! apply_player_actions。跳跃状态表用进程内 Mutex（原生插件无 wasm 线程限制）。

use std::collections::BTreeMap;
use std::sync::Mutex;

use qexed_plugin_api::PlayerInputEvent;
use qexed_plugin_sdk::{InitFuture, PluginContext};

static JUMP_STATE: Mutex<BTreeMap<String, JumpState>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Default, Clone, Copy)]
struct JumpState {
    released_jump_in_air: bool,
    used_double_jump: bool,
}

fn forward_velocity(yaw: f32, speed: f64) -> (f64, f64) {
    let radians = f64::from(yaw).to_radians();
    (-radians.sin() * speed, radians.cos() * speed)
}

qexed_plugin_sdk::declare_simple_plugin!("double_jump", "Double Jump", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        let plugin_id: &'static str = ctx.id();
        ctx.register::<PlayerInputEvent>(move |api, ev, _| {
            let plugin_id = plugin_id;
            Box::pin(async move {
                let mut states = JUMP_STATE.lock().unwrap();
                if ev.position.on_ground {
                    states.insert(ev.player.uuid.clone(), JumpState::default());
                    return;
                }
                let jump_pressed = !ev.previous_input.jump && ev.input.jump;
                if !jump_pressed {
                    if !ev.input.jump {
                        states
                            .entry(ev.player.uuid.clone())
                            .or_default()
                            .released_jump_in_air = true;
                    }
                    return;
                }
                let should_jump = {
                    let state = states.entry(ev.player.uuid.clone()).or_default();
                    if state.released_jump_in_air && !state.used_double_jump {
                        state.used_double_jump = true;
                        true
                    } else {
                        false
                    }
                };
                drop(states);
                if !should_jump {
                    return;
                }
                let (x, z) = forward_velocity(ev.position.yaw, 0.55);
                qexed_plugin_sdk::Sdk::new(api, plugin_id)
                    .perform(&ev.player, qexed_plugin_sdk::velocity(x, 0.9, z, false));
            })
        });
    })
});
