include!(concat!(env!("OUT_DIR"), "/generated.rs"));
#[unsafe(no_mangle)]
pub extern "C" fn print() {
    qexed_plugin_api::log::info("Hello World");
}
