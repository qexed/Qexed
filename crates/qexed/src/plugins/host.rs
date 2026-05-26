use wasmtime::Caller;

use super::PluginState;

const MAX_HOST_LOG_BYTES: usize = 16 * 1024;

pub(super) fn host_log(mut caller: Caller<'_, PluginState>, ptr: i32, len: i32) {
    let plugin_name = caller.data().name.clone();
    let Some(bytes) = host_memory_bytes(&mut caller, ptr, len) else {
        return;
    };
    match std::str::from_utf8(bytes) {
        Ok(message) => log::info!("[WASM 插件:{plugin_name}] {message}"),
        Err(err) => log::warn!("[WASM 插件:{plugin_name}] 日志不是 UTF-8: {err}"),
    }
}

fn host_memory_bytes<'a>(
    caller: &'a mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> Option<&'a [u8]> {
    if ptr < 0 || len < 0 {
        log::warn!("WASM 插件传入了负数内存范围: ptr={ptr}, len={len}");
        return None;
    }

    let offset = ptr as usize;
    let len = len as usize;
    if len > MAX_HOST_LOG_BYTES {
        log::warn!("WASM 插件日志过长: len={len}, max={MAX_HOST_LOG_BYTES}");
        return None;
    }

    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        log::warn!("WASM 插件调用日志 API 时缺少 memory 导出");
        return None;
    };
    let data = memory.data(&*caller);
    let end = offset.checked_add(len)?;
    if end > data.len() {
        log::warn!("WASM 插件日志内存越界: ptr={ptr}, len={len}");
        return None;
    }
    Some(&data[offset..end])
}
