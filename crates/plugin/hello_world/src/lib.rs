#[unsafe(no_mangle)]
pub extern "C" fn print() {
    qexed_plugin_api::log::info("测试");
}