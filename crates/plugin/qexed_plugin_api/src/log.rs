#[link(wasm_import_module = "log")]
unsafe extern "C" {
    fn Trace(buf: *const u8, len: i32);
    fn Info(buf: *const u8, len: i32);
    fn Debug(buf: *const u8, len: i32);
    fn Warn(buf: *const u8, len: i32);
    fn Error(buf: *const u8, len: i32);
}
pub fn trace(message:&str){
    unsafe {
        Trace(message.as_ptr(), message.len() as i32);
    }
}
pub fn info(message:&str){
    unsafe {
        Info(message.as_ptr(), message.len() as i32);
    }
}
pub fn debug(message:&str){
    unsafe {
        Debug(message.as_ptr(), message.len() as i32);
    }
}
pub fn warn(message:&str){
    unsafe {
        Warn(message.as_ptr(), message.len() as i32);
    }
}
pub fn error(message:&str){
    unsafe {
        Error(message.as_ptr(), message.len() as i32);
    }
}
