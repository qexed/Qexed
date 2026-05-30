use std::{mem, slice};

use serde::{Serialize, de::DeserializeOwned};

#[link(wasm_import_module = "qexed")]
unsafe extern "C" {
    #[link_name = "log"]
    fn host_log(ptr: i32, len: i32);
    #[link_name = "config_exists"]
    fn host_config_exists(ptr: i32, len: i32) -> i32;
    #[link_name = "config_read"]
    fn host_config_read(path_ptr: i32, path_len: i32, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "config_write"]
    fn host_config_write(path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32) -> i32;
    #[link_name = "economy_register_currency"]
    fn host_economy_register_currency(
        id_ptr: i32,
        id_len: i32,
        name_ptr: i32,
        name_len: i32,
        symbol_ptr: i32,
        symbol_len: i32,
        fractional_digits: i32,
    ) -> i32;
    #[link_name = "economy_currency_info"]
    fn host_economy_currency_info(
        currency_ptr: i32,
        currency_len: i32,
        out_ptr: i32,
        out_len: i32,
    ) -> i64;
    #[link_name = "economy_balance"]
    fn host_economy_balance(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
    ) -> i64;
    #[link_name = "economy_set_balance"]
    fn host_economy_set_balance(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
        amount: i64,
    ) -> i64;
    #[link_name = "economy_deposit"]
    fn host_economy_deposit(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
        amount: i64,
    ) -> i64;
    #[link_name = "economy_withdraw"]
    fn host_economy_withdraw(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
        amount: i64,
    ) -> i64;
}

const HOST_READ_INITIAL_BYTES: usize = 4096;
const ECONOMY_ERROR: i64 = i64::MIN + 1;

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

pub mod payload {
    pub use qexed_plugin_api::*;
}

pub use qexed_plugin_api::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyInfo {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub fractional_digits: i32,
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

pub fn config_exists(path: &str) -> bool {
    let Some(path_len) = i32_len(path.as_bytes()) else {
        return false;
    };
    unsafe { host_config_exists(path.as_ptr() as i32, path_len) == 1 }
}

pub fn config_read(path: &str) -> Option<Vec<u8>> {
    let path_len = i32_len(path.as_bytes())?;
    read_host_buffer(|out_ptr, out_len| unsafe {
        host_config_read(path.as_ptr() as i32, path_len, out_ptr, out_len)
    })
}

pub fn config_read_to_string(path: &str) -> Option<String> {
    String::from_utf8(config_read(path)?).ok()
}

pub fn config_write(path: &str, contents: &str) -> bool {
    config_write_bytes(path, contents.as_bytes())
}

pub fn config_write_bytes(path: &str, contents: &[u8]) -> bool {
    let Some(path_len) = i32_len(path.as_bytes()) else {
        return false;
    };
    let Some(contents_len) = i32_len(contents) else {
        return false;
    };
    unsafe {
        host_config_write(
            path.as_ptr() as i32,
            path_len,
            contents.as_ptr() as i32,
            contents_len,
        ) == 0
    }
}

pub fn config_load_or_create(path: &str, default_contents: &str) -> String {
    if let Some(contents) = config_read_to_string(path) {
        return contents;
    }
    let _ = config_write(path, default_contents);
    default_contents.to_string()
}

pub fn economy_register_currency(
    id: &str,
    name: &str,
    symbol: &str,
    fractional_digits: i32,
) -> bool {
    let Some(id_len) = i32_len(id.as_bytes()) else {
        return false;
    };
    let Some(name_len) = i32_len(name.as_bytes()) else {
        return false;
    };
    let Some(symbol_len) = i32_len(symbol.as_bytes()) else {
        return false;
    };
    unsafe {
        host_economy_register_currency(
            id.as_ptr() as i32,
            id_len,
            name.as_ptr() as i32,
            name_len,
            symbol.as_ptr() as i32,
            symbol_len,
            fractional_digits,
        ) == 0
    }
}

pub fn economy_currency_info(currency: &str) -> Option<CurrencyInfo> {
    let currency_len = i32_len(currency.as_bytes())?;
    let bytes = read_host_buffer(|out_ptr, out_len| unsafe {
        host_economy_currency_info(currency.as_ptr() as i32, currency_len, out_ptr, out_len)
    })?;
    let encoded = String::from_utf8(bytes).ok()?;
    let mut parts = encoded.splitn(4, '\t');
    Some(CurrencyInfo {
        id: parts.next()?.to_string(),
        name: parts.next()?.to_string(),
        symbol: parts.next()?.to_string(),
        fractional_digits: parts.next()?.parse().ok()?,
    })
}

pub fn economy_balance(player: &str, currency: &str) -> Option<i64> {
    economy_call(player, currency, |player_ptr, player_len, currency_ptr, currency_len| unsafe {
        host_economy_balance(player_ptr, player_len, currency_ptr, currency_len)
    })
}

pub fn economy_set_balance(player: &str, currency: &str, amount: i64) -> Option<i64> {
    economy_call(player, currency, |player_ptr, player_len, currency_ptr, currency_len| unsafe {
        host_economy_set_balance(player_ptr, player_len, currency_ptr, currency_len, amount)
    })
}

pub fn economy_deposit(player: &str, currency: &str, amount: i64) -> Option<i64> {
    economy_call(player, currency, |player_ptr, player_len, currency_ptr, currency_len| unsafe {
        host_economy_deposit(player_ptr, player_len, currency_ptr, currency_len, amount)
    })
}

pub fn economy_withdraw(player: &str, currency: &str, amount: i64) -> Option<i64> {
    economy_call(player, currency, |player_ptr, player_len, currency_ptr, currency_len| unsafe {
        host_economy_withdraw(player_ptr, player_len, currency_ptr, currency_len, amount)
    })
}

pub unsafe fn payload_bytes<'a>(ptr: i32, len: i32) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr <= 0 || len < 0 {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) })
}

pub unsafe fn decode_payload<T: DeserializeOwned>(ptr: i32, len: i32) -> Option<T> {
    let bytes = unsafe { payload_bytes(ptr, len) }?;
    postcard::from_bytes(bytes).ok()
}

pub fn response_ptr_len<T: Serialize>(response: &T) -> i64 {
    let Ok(bytes) = postcard::to_allocvec(response) else {
        return 0;
    };
    response_bytes_ptr_len(&bytes)
}

pub fn response_bytes_ptr_len(response: &[u8]) -> i64 {
    let len = match i32::try_from(response.len()) {
        Ok(len) if len > 0 => len,
        _ => return 0,
    };
    let ptr = alloc(len);
    if ptr <= 0 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(response.as_ptr(), ptr as *mut u8, len as usize);
    }
    ((ptr as i64) << 32) | (len as u32 as i64)
}

fn i32_len(bytes: &[u8]) -> Option<i32> {
    i32::try_from(bytes.len()).ok()
}

fn read_host_buffer(mut read: impl FnMut(i32, i32) -> i64) -> Option<Vec<u8>> {
    let mut buffer = vec![0; HOST_READ_INITIAL_BYTES];
    let len = read(buffer.as_mut_ptr() as i32, i32_len(&buffer)?);
    if len >= 0 {
        let len = usize::try_from(len).ok()?;
        buffer.truncate(len);
        return Some(buffer);
    }

    let needed = required_host_buffer_len(len)?;
    let mut buffer = vec![0; needed];
    let len = read(buffer.as_mut_ptr() as i32, i32_len(&buffer)?);
    if len < 0 {
        return None;
    }
    let len = usize::try_from(len).ok()?;
    if len > buffer.len() {
        return None;
    }
    buffer.truncate(len);
    Some(buffer)
}

fn required_host_buffer_len(ret: i64) -> Option<usize> {
    if ret >= -1 {
        return None;
    }
    usize::try_from(-ret - 1).ok()
}

fn economy_call(
    player: &str,
    currency: &str,
    call: impl FnOnce(i32, i32, i32, i32) -> i64,
) -> Option<i64> {
    let player_len = i32_len(player.as_bytes())?;
    let currency_len = i32_len(currency.as_bytes())?;
    let value = call(
        player.as_ptr() as i32,
        player_len,
        currency.as_ptr() as i32,
        currency_len,
    );
    if value <= ECONOMY_ERROR {
        None
    } else {
        Some(value)
    }
}
