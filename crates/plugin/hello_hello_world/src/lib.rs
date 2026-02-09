use std::ffi::{CStr, CString, c_char};

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
#[link(wasm_import_module = "qexed_world")]
unsafe extern "C" {
    fn print();
}
#[unsafe(no_mangle)]
pub extern "C" fn print2() {
    unsafe { print() };
}

#[unsafe(no_mangle)]
pub extern "C" fn get_plugin_display_name(language: *const c_char) -> *mut c_char {
    // 安全检查：确保指针不为空
    if language.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        // 将 C 字符串指针转换为 Rust 的 &CStr
        let lang_cstr = CStr::from_ptr(language);
        // 尝试将 &CStr 转换为 &str（UTF-8 验证）
        let _lang_str = match lang_cstr.to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(), // 处理编码错误
        };

        // ... 这里可以根据 lang_str 进行逻辑判断 ...
        // 例如，如果 lang_str 是 "zh-CN"，返回中文显示名

        // 创建一个要返回给 C 的字符串
        let display_name = "你好，世界";
        // 将 Rust 字符串转换为 C 字符串（CString），并获取其原始指针，移交所有权
        CString::new(display_name).unwrap().into_raw()
    }
}

// 提供一个配套的释放函数，让调用者用来释放内存
#[unsafe(no_mangle)]
pub extern "C" fn free_string(s: *mut c_char) {
    unsafe {
        if !s.is_null() {
            // 将指针重新转换为 CString，离开作用域时其内存会被自动释放
            let _ = CString::from_raw(s);
        }
    }
}