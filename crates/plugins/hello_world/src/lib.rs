// #[link(wasm_import_module = "env")]
// unsafe extern "C" {
//     fn write(fd: i32, buf: *const u8, len: i32) -> i32;
// }

// #[unsafe(no_mangle)]
// pub extern "C" fn print() {
//     let message = "Hello from WASM!\n";
//     unsafe {
//         write(1, message.as_ptr(), message.len() as i32);
//     }
// }

// use qexed_api::plugin;
// pub struct Plugin{
//     // 此变量若有变化,则会每秒同步一次,
//     // 这将会在所有使用此插件的模块中进行全局信息同步
//     // 请尽可能的避免或按需同步
//     #[qexed::shared(Duration::from_millis(1000))]
//     pub now_time:Duration,// 当前时间
//     pub now_node_time:Duration,// 当前节点时间
// }