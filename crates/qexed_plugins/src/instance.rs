//! 动态库插件实例（v4 plugins/instance.rs 迁移；WASM → libloading dll/so）。

//!
//! v6 插件 ABI（C ABI，跨平台 dll/so）：
//! 插件需导出：
//! - `fn qexed_plugin_host_table() -> *const c_void`：返回宿主注入的
//!   HostContext 表指针（宿主在加载后立即回填，插件侧保存）。
//! - `fn qexed_plugin_alloc(len: i32) -> *mut u8` / `fn qexed_plugin_dealloc(ptr: *mut u8, len: i32)`：
//!   事件 payload 缓冲分配/归还。
//! - 每个支持的事件：`fn qexed_plugin_<event>(ptr: *const u8, len: usize)`（fire）
//!   或 `fn qexed_plugin_<event>(ptr: *const u8, len: usize) -> *mut PluginBuffer`（query，
//!   返回 NULL 表示无结果，缓冲由宿主经 free_response 归还——与宿主 HostBuffer 同构）。
//! - 可选：`fn qexed_plugin_priority() -> i32`、`fn qexed_plugin_manifest() -> *mut PluginBuffer`。

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use libloading::{Library, Symbol};

use crate::api::PluginEvent;
use crate::host::{HostBuffer, HostContext, HostVTable, PluginHostServices};

const _: &HostVTable = &crate::host::HOST_VTABLE;
use crate::api::PluginManifest;
use crate::error::PluginsError;

const MAX_EVENT_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_QUERY_RESPONSE_BYTES: usize = 1024 * 1024;

pub(crate) struct PluginInstance {
    pub(crate) name: String,
    pub(crate) manifest: PluginManifest,
    pub(crate) priority: i32,
    pub(crate) supported_events: HashSet<PluginEvent>,
    instance: InstanceFuncs,
    /// Library 必须最后 drop（先卸载函数指针再卸载库）。
    library: Library,
}

/// 从动态库解析出的函数集。
struct InstanceFuncs {
    alloc: unsafe extern "C" fn(len: usize) -> *mut u8,
    dealloc: Option<unsafe extern "C" fn(ptr: *mut u8, len: usize)>,
}

/// 宿主侧事件回调类型。
type EventFn = unsafe extern "C" fn(ptr: *const u8, len: usize);
/// 宿主侧查询回调类型（返回缓冲或 NULL）。
type QueryFn = unsafe extern "C" fn(ptr: *const u8, len: usize) -> *mut HostBuffer;

impl PluginInstance {
    pub(crate) fn load(
        path: PathBuf,
        _services: Arc<PluginHostServices>,
    ) -> Result<Self, PluginsError> {
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin")
            .to_string();

        unsafe {
            let library = Library::new(&path)?;

            // host table：插件拿到宿主能力表的入口。
            let host_table: Symbol<unsafe extern "C" fn() -> *mut HostContext> =
                library
                    .get(b"qexed_plugin_host_table")
                    .map_err(|_| PluginsError::MissingExport {
                        plugin: name.clone(),
                        symbol: "qexed_plugin_host_table".to_string(),
                    })?;
            let host_table_ptr = host_table();
            if host_table_ptr.is_null() {
                return Err(PluginsError::MissingExport {
                    plugin: name.clone(),
                    symbol: "qexed_plugin_host_table (returned null)".to_string(),
                });
            }
            // 回填宿主上下文：插件名 + vtable。
            let name_bytes = name.clone().into_bytes().into_boxed_slice();
            let name_ptr = Box::leak(name_bytes.clone());
            std::ptr::write(
                host_table_ptr,
                HostContext {
                    plugin: name_ptr.as_ptr(),
                    plugin_len: name_ptr.len(),
                    vtable: &super::host::HOST_VTABLE,
                },
            );

            // 探测支持的事件：逐个尝试解析查询/事件签名。
            let mut supported_events = HashSet::new();
            for event in PluginEvent::ALL {
                let symbol = event.symbol();
                let has_query: bool = library
                    .get::<QueryFn>(symbol.as_bytes())
                    .is_ok();
                let has_event: bool = library
                    .get::<EventFn>(symbol.as_bytes())
                    .is_ok();
                if has_query || has_event {
                    supported_events.insert(*event);
                }
            }

            let alloc: Symbol<unsafe extern "C" fn(len: usize) -> *mut u8> = library
                .get(b"qexed_plugin_alloc")
                .map_err(|_| PluginsError::MissingExport {
                    plugin: name.clone(),
                    symbol: "qexed_plugin_alloc".to_string(),
                })?;
            let dealloc = library
                .get::<unsafe extern "C" fn(ptr: *mut u8, len: usize)>(b"qexed_plugin_dealloc")
                .ok()
                .map(|symbol| *symbol);
            let instance = InstanceFuncs {
                alloc: *alloc,
                dealloc,
            };

            let priority = library
                .get::<unsafe extern "C" fn() -> i32>(b"qexed_plugin_priority")
                .ok()
                .map(|priority| priority())
                .unwrap_or(0);

            let manifest = read_manifest(&library, &name);

            Ok(Self {
                name,
                manifest,
                priority,
                supported_events,
                instance,
                library,
            })
        }
    }

    pub(crate) fn supports_event(&self, event: PluginEvent) -> bool {
        self.supported_events.contains(&event)
    }

    /// 触发事件（v4 call_event）。
    pub(crate) fn call_event(&mut self, event: PluginEvent, payload: &[u8]) -> Result<(), PluginsError> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(PluginsError::PayloadTooLarge {
                size: payload.len(),
                max: MAX_EVENT_PAYLOAD_BYTES,
            });
        }
        unsafe {
            // 优先事件签名（无返回值）；Init 可能是 fn() 形式。
            if event == PluginEvent::Init {
                if let Ok(func) = self
                    .library
                    .get::<unsafe extern "C" fn()>(event.symbol().as_bytes())
                {
                    func();
                    return Ok(());
                }
            }
            let Ok(func) = self
                .library
                .get::<EventFn>(event.symbol().as_bytes())
            else {
                return Ok(());
            };
            let (ptr, len) = self.write_payload(payload)?;
            func(ptr, len);
            self.free_payload(ptr, len);
        }
        Ok(())
    }

    /// 查询（v4 call_query）：返回 Some(bytes) 或 None（无该查询/无结果）。
    pub(crate) fn call_query(
        &mut self,
        event: PluginEvent,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>, PluginsError> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(PluginsError::PayloadTooLarge {
                size: payload.len(),
                max: MAX_EVENT_PAYLOAD_BYTES,
            });
        }
        let bytes = self.call_event_or_query_inner(event, payload)?;
        Ok(bytes)
    }

    /// 事件或查询合一（v4 call_event_or_query）。
    pub(crate) fn call_event_or_query(
        &mut self,
        event: PluginEvent,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>, PluginsError> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(PluginsError::PayloadTooLarge {
                size: payload.len(),
                max: MAX_EVENT_PAYLOAD_BYTES,
            });
        }
        self.call_event_or_query_inner(event, payload)
    }

    fn call_event_or_query_inner(
        &mut self,
        event: PluginEvent,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>, PluginsError> {
        unsafe {
            // 查询签名优先。
            if let Ok(func) = self
                .library
                .get::<QueryFn>(event.symbol().as_bytes())
            {
                let (ptr, len) = self.write_payload(payload)?;
                let response = func(ptr, len);
                self.free_payload(ptr, len);
                return read_host_buffer(response, MAX_QUERY_RESPONSE_BYTES);
            }
            // 退化成事件签名。
            if let Ok(func) = self
                .library
                .get::<EventFn>(event.symbol().as_bytes())
            {
                let (ptr, len) = self.write_payload(payload)?;
                func(ptr, len);
                self.free_payload(ptr, len);
            }
            Ok(None)
        }
    }

    /// 把 payload 写进插件分配的缓冲，返回 (ptr, len)。
    unsafe fn write_payload(&self, payload: &[u8]) -> Result<(*const u8, usize), PluginsError> {
        unsafe {
            let ptr = (self.instance.alloc)(payload.len());
            if ptr.is_null() {
                return Err(PluginsError::InvalidLength {
                    plugin: self.name.clone(),
                    value: -(payload.len() as i64),
                });
            }
            std::ptr::copy_nonoverlapping(payload.as_ptr(), ptr, payload.len());
            Ok((ptr, payload.len()))
        }
    }

    unsafe fn free_payload(&self, ptr: *const u8, len: usize) {
        if let Some(dealloc) = self.instance.dealloc {
            if !ptr.is_null() {
                unsafe { dealloc(ptr as *mut u8, len) };
            }
        }
    }
}

/// 读取宿主形状的缓冲（ptr+len），读后归还。
unsafe fn read_host_buffer(
    buffer: *mut HostBuffer,
    max: usize,
) -> Result<Option<Vec<u8>>, PluginsError> {
    if buffer.is_null() {
        return Ok(None);
    }
    unsafe {
        let host_buffer = Box::from_raw(buffer);
        if host_buffer.ptr.is_null() || host_buffer.len > max {
            drop_buffers(&host_buffer);
            if host_buffer.len > max {
                return Err(PluginsError::PayloadTooLarge {
                    size: host_buffer.len,
                    max,
                });
            }
            return Ok(None);
        }
        let bytes =
            Vec::from_raw_parts(host_buffer.ptr, host_buffer.len, host_buffer.len);
        Ok(Some(bytes))
    }
}

fn drop_buffers(buffer: &HostBuffer) {
    if !buffer.ptr.is_null() && buffer.len > 0 {
        unsafe {
            drop(Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.len));
        }
    }
}

fn read_manifest(
    library: &Library,
    name: &str,
) -> PluginManifest {
    let default = || default_manifest(name);
    unsafe {
        let Ok(func) = library
            .get::<unsafe extern "C" fn() -> *mut HostBuffer>(b"qexed_plugin_manifest")
        else {
            return default();
        };
        let buffer = func();
        if buffer.is_null() {
            return default();
        }
        // 缓冲由插件分配（v4 经 postcard 交换；v6 为 JSON），宿主读取后即拷贝。
        let host_buffer = Box::from_raw(buffer);
        if host_buffer.ptr.is_null()
            || host_buffer.len == 0
            || host_buffer.len > MAX_QUERY_RESPONSE_BYTES
        {
            // len 越界时不能安全重构 Vec，只能泄漏指针避免 double-free。
            drop_buffers(&host_buffer);
            return default();
        }
        let bytes = Vec::from_raw_parts(host_buffer.ptr, host_buffer.len, host_buffer.len);
        match serde_json::from_slice::<PluginManifest>(&bytes).ok() {
            Some(mut manifest) => {
                if manifest.id.trim().is_empty() {
                    manifest.id = name.to_string();
                }
                manifest
            }
            None => default(),
        }
    }
}
fn default_manifest(name: &str) -> PluginManifest {
    PluginManifest {
        id: name.to_string(),
        version: String::new(),
        depends: Vec::new(),
        optional_depends: Vec::new(),
        load_after: Vec::new(),
        services: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_manifest_falls_back_to_plugin_name() {
        let manifest = default_manifest("my-plugin");
        assert_eq!(manifest.id, "my-plugin");
        assert!(manifest.depends.is_empty());
    }
}

