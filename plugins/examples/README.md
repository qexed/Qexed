# Qexed WASM 插件示例

这些示例使用 `wasm32-unknown-unknown` 目标编译。编译后把生成的 `.wasm` 放到服务端运行目录的 `plugins` 文件夹下，Qexed 启动时会自动扫描 `plugins/*.wasm`。

```powershell
rustup target add wasm32-unknown-unknown
cargo build --manifest-path plugins/examples/Cargo.toml --target wasm32-unknown-unknown --release
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/hello_world.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/player_audit.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/chunk_trace.wasm" "plugins/"
Copy-Item "plugins/examples/target/wasm32-unknown-unknown/release/command_npc_demo.wasm" "plugins/"
```

运行时 ABI：

- 必须导出 `memory`
- 必须导出 `qexed_plugin_alloc(len: i32) -> i32`
- 可选导出 `qexed_plugin_dealloc(ptr: i32, len: i32)`
- 可选导出 `qexed_plugin_priority() -> i32`，数值越大越先执行
- 插件可导入宿主函数 `qexed::log(ptr: i32, len: i32)` 写入服务端日志

事件函数都是可选的。除 `qexed_plugin_init()` 外，事件参数是 JSON bytes。

`chunk_trace` 会在区块加载/卸载时写日志，视距较大时日志会很多，只适合调试 ABI。
