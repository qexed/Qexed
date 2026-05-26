use std::{mem, slice, str};

#[link(wasm_import_module = "qexed")]
unsafe extern "C" {
    #[link_name = "log"]
    fn host_log(ptr: i32, len: i32);
}

#[macro_export]
macro_rules! qexed_plugin_memory {
    () => {
        #[unsafe(no_mangle)]
        pub extern "C" fn qexed_plugin_alloc(len: i32) -> i32 {
            $crate::alloc(len)
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn qexed_plugin_dealloc(ptr: i32, len: i32) {
            unsafe {
                $crate::dealloc(ptr, len);
            }
        }
    };
}

pub fn alloc(len: i32) -> i32 {
    if len <= 0 {
        return 0;
    }
    let mut buffer = Vec::<u8>::with_capacity(len as usize);
    let ptr = buffer.as_mut_ptr();
    mem::forget(buffer);
    ptr as i32
}

pub unsafe fn dealloc(ptr: i32, len: i32) {
    if ptr <= 0 || len <= 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr as *mut u8, 0, len as usize));
    }
}

pub fn log(message: &str) {
    unsafe {
        host_log(message.as_ptr() as i32, message.len() as i32);
    }
}

pub fn response_ptr_len(response: &str) -> i64 {
    let len = response.len() as i32;
    let ptr = alloc(len);
    if ptr <= 0 || len <= 0 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(response.as_ptr(), ptr as *mut u8, len as usize);
    }
    ((ptr as i64) << 32) | (len as u32 as i64)
}

pub unsafe fn payload_str<'a>(ptr: i32, len: i32) -> Option<&'a str> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    str::from_utf8(bytes).ok()
}

pub fn json_string_field<'a>(payload: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\":\"");
    let start = payload.find(&key)? + key.len();
    let rest = &payload[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

pub fn json_i32_field(payload: &str, field: &str) -> Option<i32> {
    let key = format!("\"{field}\":");
    let start = payload.find(&key)? + key.len();
    let rest = payload[start..].trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit() && ch != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}
