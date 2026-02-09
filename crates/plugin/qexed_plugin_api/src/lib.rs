pub mod log;
#[unsafe(no_mangle)]
pub extern "C" fn allocate_string(length: u32) -> *mut u8 {
    let mut buffer = Vec::with_capacity(length as usize);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn free_string(ptr: *mut u8, length: u32) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, length as usize);
    }
}