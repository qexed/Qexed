use std::{
    future::Future,
    mem,
    pin::Pin,
    slice,
    task::{Context, Poll},
};

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
    #[link_name = "storage_exists"]
    fn host_storage_exists(key_ptr: i32, key_len: i32) -> i32;
    #[link_name = "storage_get"]
    fn host_storage_get(key_ptr: i32, key_len: i32, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "storage_set"]
    fn host_storage_set(key_ptr: i32, key_len: i32, data_ptr: i32, data_len: i32) -> i32;
    #[link_name = "storage_delete"]
    fn host_storage_delete(key_ptr: i32, key_len: i32) -> i32;
    #[link_name = "structured_storage_exists"]
    fn host_structured_storage_exists(key_ptr: i32, key_len: i32) -> i32;
    #[link_name = "structured_storage_get"]
    fn host_structured_storage_get(key_ptr: i32, key_len: i32, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "structured_storage_set"]
    fn host_structured_storage_set(key_ptr: i32, key_len: i32, data_ptr: i32, data_len: i32)
    -> i32;
    #[link_name = "structured_storage_delete"]
    fn host_structured_storage_delete(key_ptr: i32, key_len: i32) -> i32;
    #[link_name = "structured_storage_exists_async"]
    fn host_structured_storage_exists_async(key_ptr: i32, key_len: i32) -> i64;
    #[link_name = "structured_storage_get_async"]
    fn host_structured_storage_get_async(key_ptr: i32, key_len: i32) -> i64;
    #[link_name = "structured_storage_set_async"]
    fn host_structured_storage_set_async(
        key_ptr: i32,
        key_len: i32,
        data_ptr: i32,
        data_len: i32,
    ) -> i64;
    #[link_name = "structured_storage_delete_async"]
    fn host_structured_storage_delete_async(key_ptr: i32, key_len: i32) -> i64;
    #[link_name = "structured_storage_async_poll"]
    fn host_structured_storage_async_poll(id: i64, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "structured_storage_async_forget"]
    fn host_structured_storage_async_forget(id: i64) -> i32;
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
    #[link_name = "economy_storage"]
    fn host_economy_storage(
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
    #[link_name = "economy_balance_async"]
    fn host_economy_balance_async(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
    ) -> i64;
    #[link_name = "economy_set_balance_async"]
    fn host_economy_set_balance_async(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
        amount: i64,
    ) -> i64;
    #[link_name = "economy_deposit_async"]
    fn host_economy_deposit_async(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
        amount: i64,
    ) -> i64;
    #[link_name = "economy_withdraw_async"]
    fn host_economy_withdraw_async(
        player_ptr: i32,
        player_len: i32,
        currency_ptr: i32,
        currency_len: i32,
        amount: i64,
    ) -> i64;
    #[link_name = "economy_async_poll"]
    fn host_economy_async_poll(id: i64) -> i64;
    #[link_name = "economy_async_forget"]
    fn host_economy_async_forget(id: i64) -> i32;
    #[link_name = "lottery_roll"]
    fn host_lottery_roll(entries_ptr: i32, entries_len: i32, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "pathfinding_find"]
    fn host_pathfinding_find(query_ptr: i32, query_len: i32, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "geyser_player_info"]
    fn host_geyser_player_info(query_ptr: i32, query_len: i32, out_ptr: i32, out_len: i32) -> i64;
    #[link_name = "world_set_block"]
    fn host_world_set_block(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "world_set_blocks"]
    fn host_world_set_blocks(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "world_break_block"]
    fn host_world_break_block(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "world_register_edit_region"]
    fn host_world_register_edit_region(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "entity_upsert"]
    fn host_entity_upsert(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "entity_move"]
    fn host_entity_move(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "entity_remove"]
    fn host_entity_remove(query_ptr: i32, query_len: i32) -> i32;
    #[link_name = "random_pool_roll"]
    fn host_random_pool_roll(request_ptr: i32, request_len: i32, out_ptr: i32, out_len: i32)
    -> i64;
    #[link_name = "time_millis"]
    fn host_time_millis() -> i64;
    #[link_name = "plugin_service_exists"]
    fn host_plugin_service_exists(service_ptr: i32, service_len: i32) -> i32;
    #[link_name = "plugin_call"]
    fn host_plugin_call(
        service_ptr: i32,
        service_len: i32,
        method_ptr: i32,
        method_len: i32,
        payload_ptr: i32,
        payload_len: i32,
        out_ptr: i32,
        out_len: i32,
    ) -> i64;
    #[link_name = "http_request"]
    fn host_http_request(request_ptr: i32, request_len: i32, out_ptr: i32, out_len: i32) -> i64;
}

const HOST_READ_INITIAL_BYTES: usize = 4096;
const ECONOMY_ERROR: i64 = i64::MIN + 1;
const ECONOMY_ASYNC_PENDING: i64 = i64::MIN + 2;
const STRUCTURED_STORAGE_ASYNC_PENDING: i64 = i64::MIN + 2;

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

#[macro_export]
macro_rules! qexed_plugin_manifest {
    ($manifest:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn qexed_plugin_manifest(_ptr: i32, _len: i32) -> i64 {
            $crate::response_ptr_len(&$manifest)
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

pub fn time_millis() -> i64 {
    unsafe { host_time_millis() }
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

pub fn storage_exists(key: &str) -> bool {
    let Some(key_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    unsafe { host_storage_exists(key.as_ptr() as i32, key_len) == 1 }
}

pub fn storage_get(key: &str) -> Option<Vec<u8>> {
    let key_len = i32_len(key.as_bytes())?;
    read_host_buffer(|out_ptr, out_len| unsafe {
        host_storage_get(key.as_ptr() as i32, key_len, out_ptr, out_len)
    })
}

pub fn storage_set(key: &str, value: &[u8]) -> bool {
    let Some(key_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    let Some(value_len) = i32_len(value) else {
        return false;
    };
    unsafe {
        host_storage_set(
            key.as_ptr() as i32,
            key_len,
            value.as_ptr() as i32,
            value_len,
        ) == 0
    }
}

pub fn storage_delete(key: &str) -> bool {
    let Some(key_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    unsafe { host_storage_delete(key.as_ptr() as i32, key_len) == 0 }
}

pub fn storage_get_typed<T: DeserializeOwned>(key: &str) -> Option<T> {
    postcard::from_bytes(&storage_get(key)?).ok()
}

pub fn storage_set_typed<T: Serialize>(key: &str, value: &T) -> bool {
    let Ok(bytes) = postcard::to_allocvec(value) else {
        return false;
    };
    storage_set(key, &bytes)
}

pub fn structured_storage_exists(key: &str) -> bool {
    let Some(key_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    unsafe { host_structured_storage_exists(key.as_ptr() as i32, key_len) == 1 }
}

pub fn structured_storage_get(key: &str) -> Option<Vec<u8>> {
    let key_len = i32_len(key.as_bytes())?;
    read_host_buffer(|out_ptr, out_len| unsafe {
        host_structured_storage_get(key.as_ptr() as i32, key_len, out_ptr, out_len)
    })
}

pub fn structured_storage_set(key: &str, value: &[u8]) -> bool {
    let Some(key_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    let Some(value_len) = i32_len(value) else {
        return false;
    };
    unsafe {
        host_structured_storage_set(
            key.as_ptr() as i32,
            key_len,
            value.as_ptr() as i32,
            value_len,
        ) == 0
    }
}

pub fn structured_storage_delete(key: &str) -> bool {
    let Some(key_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    unsafe { host_structured_storage_delete(key.as_ptr() as i32, key_len) == 0 }
}

pub fn structured_storage_get_typed<T: DeserializeOwned>(key: &str) -> Option<T> {
    postcard::from_bytes(&structured_storage_get(key)?).ok()
}

pub fn structured_storage_set_typed<T: Serialize>(key: &str, value: &T) -> bool {
    let Ok(bytes) = postcard::to_allocvec(value) else {
        return false;
    };
    structured_storage_set(key, &bytes)
}

#[derive(Debug)]
pub struct StructuredStorageAsync {
    id: i64,
}

impl StructuredStorageAsync {
    pub fn poll_bytes(&mut self) -> Option<Option<Vec<u8>>> {
        if self.id <= 0 {
            return Some(None);
        }
        let result = read_host_buffer_with_pending(|out_ptr, out_len| unsafe {
            host_structured_storage_async_poll(self.id, out_ptr, out_len)
        });
        match result {
            HostReadPoll::Pending => None,
            HostReadPoll::Ready(bytes) => {
                self.id = 0;
                Some(Some(bytes))
            }
            HostReadPoll::Failed => {
                self.id = 0;
                Some(None)
            }
        }
    }

    pub fn poll_bool(&mut self) -> Option<bool> {
        self.poll_bytes().map(|bytes| {
            bytes
                .and_then(|bytes| bytes.first().copied())
                .map(|value| value == 1)
                .unwrap_or(false)
        })
    }

    pub fn poll_typed<T: DeserializeOwned>(&mut self) -> Option<Option<T>> {
        self.poll_bytes()
            .map(|bytes| bytes.and_then(|bytes| postcard::from_bytes(&bytes).ok()))
    }

    pub fn id(&self) -> i64 {
        self.id
    }
}

impl Future for StructuredStorageAsync {
    type Output = Option<Vec<u8>>;

    fn poll(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.poll_bytes() {
            Some(result) => Poll::Ready(result),
            None => {
                _context.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

impl Drop for StructuredStorageAsync {
    fn drop(&mut self) {
        if self.id > 0 {
            let _ = unsafe { host_structured_storage_async_forget(self.id) };
            self.id = 0;
        }
    }
}

pub fn structured_storage_exists_async(key: &str) -> Option<StructuredStorageAsync> {
    let key_len = i32_len(key.as_bytes())?;
    let id = unsafe { host_structured_storage_exists_async(key.as_ptr() as i32, key_len) };
    (id > 0).then_some(StructuredStorageAsync { id })
}

pub fn structured_storage_get_async(key: &str) -> Option<StructuredStorageAsync> {
    let key_len = i32_len(key.as_bytes())?;
    let id = unsafe { host_structured_storage_get_async(key.as_ptr() as i32, key_len) };
    (id > 0).then_some(StructuredStorageAsync { id })
}

pub fn structured_storage_set_async(key: &str, value: &[u8]) -> Option<StructuredStorageAsync> {
    let key_len = i32_len(key.as_bytes())?;
    let value_len = i32_len(value)?;
    let id = unsafe {
        host_structured_storage_set_async(
            key.as_ptr() as i32,
            key_len,
            value.as_ptr() as i32,
            value_len,
        )
    };
    (id > 0).then_some(StructuredStorageAsync { id })
}

pub fn structured_storage_delete_async(key: &str) -> Option<StructuredStorageAsync> {
    let key_len = i32_len(key.as_bytes())?;
    let id = unsafe { host_structured_storage_delete_async(key.as_ptr() as i32, key_len) };
    (id > 0).then_some(StructuredStorageAsync { id })
}

pub fn structured_storage_set_typed_async<T: Serialize>(
    key: &str,
    value: &T,
) -> Option<StructuredStorageAsync> {
    let Ok(bytes) = postcard::to_allocvec(value) else {
        return None;
    };
    structured_storage_set_async(key, &bytes)
}

pub struct StructuredStorageTypedAsync<T> {
    inner: StructuredStorageAsync,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Unpin for StructuredStorageTypedAsync<T> {}

impl<T: DeserializeOwned> StructuredStorageTypedAsync<T> {
    pub fn poll_result(&mut self) -> Option<Option<T>> {
        self.inner.poll_typed()
    }
}

impl<T: DeserializeOwned> Future for StructuredStorageTypedAsync<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.get_mut().inner.poll_typed() {
            Some(result) => Poll::Ready(result),
            None => {
                _context.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

pub fn structured_storage_get_typed_async<T: DeserializeOwned>(
    key: &str,
) -> Option<StructuredStorageTypedAsync<T>> {
    Some(StructuredStorageTypedAsync {
        inner: structured_storage_get_async(key)?,
        _marker: std::marker::PhantomData,
    })
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

pub fn economy_storage(currency: &str) -> Option<String> {
    let currency_len = i32_len(currency.as_bytes())?;
    let bytes = read_host_buffer(|out_ptr, out_len| unsafe {
        host_economy_storage(currency.as_ptr() as i32, currency_len, out_ptr, out_len)
    })?;
    String::from_utf8(bytes).ok()
}

pub fn economy_balance(player: &str, currency: &str) -> Option<i64> {
    economy_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_balance(player_ptr, player_len, currency_ptr, currency_len)
        },
    )
}

pub fn economy_set_balance(player: &str, currency: &str, amount: i64) -> Option<i64> {
    economy_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_set_balance(player_ptr, player_len, currency_ptr, currency_len, amount)
        },
    )
}

pub fn economy_deposit(player: &str, currency: &str, amount: i64) -> Option<i64> {
    economy_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_deposit(player_ptr, player_len, currency_ptr, currency_len, amount)
        },
    )
}

pub fn economy_withdraw(player: &str, currency: &str, amount: i64) -> Option<i64> {
    economy_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_withdraw(player_ptr, player_len, currency_ptr, currency_len, amount)
        },
    )
}

#[derive(Debug)]
pub struct EconomyAsync {
    id: i64,
}

impl EconomyAsync {
    pub fn poll_result(&mut self) -> Option<Option<i64>> {
        if self.id <= 0 {
            return Some(None);
        }
        let result = unsafe { host_economy_async_poll(self.id) };
        if result == ECONOMY_ASYNC_PENDING {
            return None;
        }
        self.id = 0;
        Some(economy_result(result))
    }

    pub fn id(&self) -> i64 {
        self.id
    }
}

impl Future for EconomyAsync {
    type Output = Option<i64>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.poll_result() {
            Some(result) => Poll::Ready(result),
            None => {
                context.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

impl Drop for EconomyAsync {
    fn drop(&mut self) {
        if self.id > 0 {
            let _ = unsafe { host_economy_async_forget(self.id) };
            self.id = 0;
        }
    }
}

pub fn economy_balance_async(player: &str, currency: &str) -> Option<EconomyAsync> {
    economy_async_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_balance_async(player_ptr, player_len, currency_ptr, currency_len)
        },
    )
}

pub fn economy_set_balance_async(
    player: &str,
    currency: &str,
    amount: i64,
) -> Option<EconomyAsync> {
    economy_async_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_set_balance_async(
                player_ptr,
                player_len,
                currency_ptr,
                currency_len,
                amount,
            )
        },
    )
}

pub fn economy_deposit_async(player: &str, currency: &str, amount: i64) -> Option<EconomyAsync> {
    economy_async_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_deposit_async(player_ptr, player_len, currency_ptr, currency_len, amount)
        },
    )
}

pub fn economy_withdraw_async(player: &str, currency: &str, amount: i64) -> Option<EconomyAsync> {
    economy_async_call(
        player,
        currency,
        |player_ptr, player_len, currency_ptr, currency_len| unsafe {
            host_economy_withdraw_async(player_ptr, player_len, currency_ptr, currency_len, amount)
        },
    )
}

pub fn lottery_roll(entries: &[(&str, u64)]) -> Option<String> {
    let mut encoded = String::new();
    for (value, weight) in entries {
        if *weight == 0 || value.trim().is_empty() {
            continue;
        }
        encoded.push_str(&weight.to_string());
        encoded.push('\t');
        encoded.push_str(value.trim());
        encoded.push('\n');
    }
    let encoded_len = i32_len(encoded.as_bytes())?;
    let bytes = read_host_buffer(|out_ptr, out_len| unsafe {
        host_lottery_roll(encoded.as_ptr() as i32, encoded_len, out_ptr, out_len)
    })?;
    String::from_utf8(bytes).ok()
}

pub fn pathfinding_find(
    dimension: &str,
    start: (i32, i32, i32),
    goal: (i32, i32, i32),
    max_nodes: usize,
) -> Option<Vec<(i32, i32, i32)>> {
    let query = format!(
        "{} {} {} {} {} {} {} {}",
        dimension, start.0, start.1, start.2, goal.0, goal.1, goal.2, max_nodes
    );
    let query_len = i32_len(query.as_bytes())?;
    let bytes = read_host_buffer(|out_ptr, out_len| unsafe {
        host_pathfinding_find(query.as_ptr() as i32, query_len, out_ptr, out_len)
    })?;
    let encoded = String::from_utf8(bytes).ok()?;
    encoded
        .split(';')
        .map(|point| {
            let mut parts = point.split(',');
            Some((
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            ))
        })
        .collect()
}

pub fn geyser_player_info(query: &GeyserPlayerInfoQuery) -> Option<GeyserPlayerInfoResponse> {
    let bytes = postcard::to_allocvec(query).ok()?;
    let query_len = i32_len(&bytes)?;
    let response = read_host_buffer(|out_ptr, out_len| unsafe {
        host_geyser_player_info(bytes.as_ptr() as i32, query_len, out_ptr, out_len)
    })?;
    postcard::from_bytes(&response).ok()
}

pub fn world_set_block(dimension: &str, position: (i32, i32, i32), block: &str) -> bool {
    if block.trim().is_empty() {
        return false;
    }
    let query = format!(
        "{} {} {} {} {}",
        dimension, position.0, position.1, position.2, block
    );
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_world_set_block(query.as_ptr() as i32, query_len) == 0 }
}

pub fn world_set_blocks<'a>(
    blocks: impl IntoIterator<Item = (&'a str, (i32, i32, i32), &'a str)>,
) -> bool {
    let mut query = String::new();
    for (dimension, position, block) in blocks {
        if dimension.trim().is_empty() || block.trim().is_empty() {
            return false;
        }
        query.push_str(dimension.trim());
        query.push(' ');
        query.push_str(&position.0.to_string());
        query.push(' ');
        query.push_str(&position.1.to_string());
        query.push(' ');
        query.push_str(&position.2.to_string());
        query.push(' ');
        query.push_str(block.trim());
        query.push('\n');
    }
    if query.is_empty() {
        return true;
    }
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_world_set_blocks(query.as_ptr() as i32, query_len) == 0 }
}

pub fn world_break_block(dimension: &str, position: (i32, i32, i32)) -> bool {
    let query = format!("{} {} {} {}", dimension, position.0, position.1, position.2);
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_world_break_block(query.as_ptr() as i32, query_len) == 0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEditRegion<'a> {
    pub id: &'a str,
    pub dimension: &'a str,
    pub min: (i32, i32, i32),
    pub max: (i32, i32, i32),
    pub allow_player_break: bool,
    pub allow_player_place: bool,
    pub allow_plugin_write: bool,
    pub runtime_only: bool,
}

pub fn world_register_edit_region(region: &WorldEditRegion<'_>) -> bool {
    if region.id.trim().is_empty() || region.dimension.trim().is_empty() {
        return false;
    }
    let query = format!(
        "{} {} {} {} {} {} {} {} {} {} {} {}",
        region.id.trim(),
        region.dimension.trim(),
        region.min.0,
        region.max.0,
        region.min.1,
        region.max.1,
        region.min.2,
        region.max.2,
        bool_flag(region.allow_player_break),
        bool_flag(region.allow_player_place),
        bool_flag(region.allow_plugin_write),
        bool_flag(region.runtime_only),
    );
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_world_register_edit_region(query.as_ptr() as i32, query_len) == 0 }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeEntity<'a> {
    pub key: &'a str,
    pub dimension: &'a str,
    pub entity_type: &'a str,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub display_name: &'a str,
    pub ai: &'a str,
    pub ai_params_json: &'a str,
    pub auto_jump: bool,
}

pub fn entity_upsert(entity: &RuntimeEntity<'_>) -> bool {
    if !valid_runtime_entity_key(entity.key)
        || entity.dimension.trim().is_empty()
        || entity.entity_type.trim().is_empty()
    {
        return false;
    }
    let query = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        entity.key.trim(),
        entity.dimension.trim(),
        entity.entity_type.trim(),
        entity.x,
        entity.y,
        entity.z,
        entity.yaw,
        entity.pitch,
        entity.display_name.trim(),
        entity.ai.trim(),
        entity.ai_params_json.trim(),
        bool_flag(entity.auto_jump),
    );
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_entity_upsert(query.as_ptr() as i32, query_len) == 0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEntityEvents<'a> {
    pub main_hand_event: &'a str,
    pub off_hand_event: &'a str,
    pub attack_event: &'a str,
}

pub fn entity_upsert_with_events(
    entity: &RuntimeEntity<'_>,
    events: &RuntimeEntityEvents<'_>,
) -> bool {
    if !valid_runtime_entity_key(entity.key)
        || entity.dimension.trim().is_empty()
        || entity.entity_type.trim().is_empty()
    {
        return false;
    }
    let query = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        entity.key.trim(),
        entity.dimension.trim(),
        entity.entity_type.trim(),
        entity.x,
        entity.y,
        entity.z,
        entity.yaw,
        entity.pitch,
        entity.display_name.trim(),
        entity.ai.trim(),
        entity.ai_params_json.trim(),
        bool_flag(entity.auto_jump),
        events.main_hand_event.trim(),
        events.off_hand_event.trim(),
        events.attack_event.trim(),
    );
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_entity_upsert(query.as_ptr() as i32, query_len) == 0 }
}

pub fn entity_move(
    key: &str,
    dimension: &str,
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
) -> bool {
    if !valid_runtime_entity_key(key) || dimension.trim().is_empty() {
        return false;
    }
    let query = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        key.trim(),
        dimension.trim(),
        x,
        y,
        z,
        yaw,
        pitch
    );
    let Some(query_len) = i32_len(query.as_bytes()) else {
        return false;
    };
    unsafe { host_entity_move(query.as_ptr() as i32, query_len) == 0 }
}

pub fn entity_remove(key: &str) -> bool {
    if !valid_runtime_entity_key(key) {
        return false;
    }
    let key = key.trim();
    let Some(query_len) = i32_len(key.as_bytes()) else {
        return false;
    };
    unsafe { host_entity_remove(key.as_ptr() as i32, query_len) == 0 }
}

fn valid_runtime_entity_key(key: &str) -> bool {
    let key = key.trim();
    !key.is_empty()
        && key.len() <= 128
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/'))
}

pub fn random_block_pool_roll(
    pool_id: &str,
    entries: &[(&str, u64)],
    precompute_count: usize,
) -> Option<String> {
    random_pool_roll("block", pool_id, entries, precompute_count)
}

pub fn random_item_pool_roll(
    pool_id: &str,
    entries: &[(&str, u64)],
    precompute_count: usize,
) -> Option<String> {
    random_pool_roll("item", pool_id, entries, precompute_count)
}

pub fn random_chest_item_pool_roll(
    pool_id: &str,
    entries: &[(&str, u64)],
    precompute_count: usize,
) -> Option<String> {
    random_pool_roll("chest_item", pool_id, entries, precompute_count)
}

pub fn random_pool_roll(
    kind: &str,
    pool_id: &str,
    entries: &[(&str, u64)],
    precompute_count: usize,
) -> Option<String> {
    if kind.trim().is_empty() || pool_id.trim().is_empty() {
        return None;
    }
    let mut encoded = format!(
        "{}\t{}\t{}\n",
        pool_id.trim(),
        kind.trim(),
        precompute_count
    );
    for (value, weight) in entries {
        if *weight == 0 || value.trim().is_empty() {
            continue;
        }
        encoded.push_str(&weight.to_string());
        encoded.push('\t');
        encoded.push_str(value.trim());
        encoded.push('\n');
    }
    let encoded_len = i32_len(encoded.as_bytes())?;
    let bytes = read_host_buffer(|out_ptr, out_len| unsafe {
        host_random_pool_roll(encoded.as_ptr() as i32, encoded_len, out_ptr, out_len)
    })?;
    String::from_utf8(bytes).ok()
}

pub fn plugin_service_exists(service: &str) -> bool {
    let Some(service_len) = i32_len(service.as_bytes()) else {
        return false;
    };
    unsafe { host_plugin_service_exists(service.as_ptr() as i32, service_len) == 1 }
}

pub fn plugin_call(service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
    let service_len = i32_len(service.as_bytes())?;
    let method_len = i32_len(method.as_bytes())?;
    let payload_len = i32_len(payload)?;
    read_host_buffer(|out_ptr, out_len| unsafe {
        host_plugin_call(
            service.as_ptr() as i32,
            service_len,
            method.as_ptr() as i32,
            method_len,
            payload.as_ptr() as i32,
            payload_len,
            out_ptr,
            out_len,
        )
    })
}

pub fn http_request(request: &HttpRequest) -> Option<HttpResponse> {
    let bytes = postcard::to_allocvec(request).ok()?;
    let request_len = i32_len(&bytes)?;
    let response = read_host_buffer(|out_ptr, out_len| unsafe {
        host_http_request(bytes.as_ptr() as i32, request_len, out_ptr, out_len)
    })?;
    postcard::from_bytes(&response).ok()
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

enum HostReadPoll {
    Pending,
    Ready(Vec<u8>),
    Failed,
}

fn read_host_buffer_with_pending(mut read: impl FnMut(i32, i32) -> i64) -> HostReadPoll {
    let mut buffer = vec![0; HOST_READ_INITIAL_BYTES];
    let len = match i32_len(&buffer) {
        Some(buffer_len) => read(buffer.as_mut_ptr() as i32, buffer_len),
        None => return HostReadPoll::Failed,
    };
    if len == STRUCTURED_STORAGE_ASYNC_PENDING {
        return HostReadPoll::Pending;
    }
    if len >= 0 {
        let Ok(len) = usize::try_from(len) else {
            return HostReadPoll::Failed;
        };
        buffer.truncate(len);
        return HostReadPoll::Ready(buffer);
    }

    let Some(needed) = required_host_buffer_len(len) else {
        return HostReadPoll::Failed;
    };
    let mut buffer = vec![0; needed];
    let len = match i32_len(&buffer) {
        Some(buffer_len) => read(buffer.as_mut_ptr() as i32, buffer_len),
        None => return HostReadPoll::Failed,
    };
    if len == STRUCTURED_STORAGE_ASYNC_PENDING {
        return HostReadPoll::Pending;
    }
    if len < 0 {
        return HostReadPoll::Failed;
    }
    let Ok(len) = usize::try_from(len) else {
        return HostReadPoll::Failed;
    };
    if len > buffer.len() {
        return HostReadPoll::Failed;
    }
    buffer.truncate(len);
    HostReadPoll::Ready(buffer)
}

fn required_host_buffer_len(ret: i64) -> Option<usize> {
    if ret >= -1 {
        return None;
    }
    usize::try_from(-ret - 1).ok()
}

fn bool_flag(value: bool) -> &'static str {
    if value { "1" } else { "0" }
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
    economy_result(value)
}

fn economy_async_call(
    player: &str,
    currency: &str,
    call: impl FnOnce(i32, i32, i32, i32) -> i64,
) -> Option<EconomyAsync> {
    let player_len = i32_len(player.as_bytes())?;
    let currency_len = i32_len(currency.as_bytes())?;
    let id = call(
        player.as_ptr() as i32,
        player_len,
        currency.as_ptr() as i32,
        currency_len,
    );
    (id > 0).then_some(EconomyAsync { id })
}

fn economy_result(value: i64) -> Option<i64> {
    (value > ECONOMY_ERROR).then_some(value)
}
