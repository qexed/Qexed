# Qexed WASM plugin examples

These examples target `wasm32-unknown-unknown`. After building, copy the generated `.wasm` files into the server runtime `plugins` directory. Qexed scans `plugins/*.wasm` on startup.

```powershell
rustup target add wasm32-unknown-unknown
cargo build --manifest-path "plugins/examples/Cargo.toml" --target wasm32-unknown-unknown --release
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/hello_world.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/player_audit.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/chunk_trace.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/command_npc_demo.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/slime_jump.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/double_jump.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/creature_game.wasm" "plugins/"
```

Runtime ABI:
- Export `memory`.
- Export `qexed_plugin_alloc(len: i32) -> i32`.
- Optionally export `qexed_plugin_dealloc(ptr: i32, len: i32)`.
- Optionally export `qexed_plugin_priority() -> i32`; larger values run first.
- Plugins can import `qexed::log(ptr: i32, len: i32)` to write server logs.

Event functions are optional. Except for `qexed_plugin_init()`, event payloads and query responses use the typed postcard binary structures exposed by `qexed_plugin_sdk`, not JSON.

`chunk_trace` logs chunk load/unload events and can produce a lot of output when view distance is high. Use it only for ABI debugging.

`slime_jump` listens for `qexed_plugin_player_block_step` and launches players from `minecraft:light_weighted_pressure_plate` toward the configured target position. `minecraft:slime_block` remains enabled as a legacy trigger.

`double_jump` listens for `qexed_plugin_player_move` and applies one extra forward/upward velocity boost while the player is airborne.

`creature_game` implements the 浅屿闲游 creature minigame with per-player private arenas and a menu-driven new-game/lobby flow.
