use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};
use serde::Serialize;
use wasmtime::{Caller, Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

use crate::players::OnlinePlayer;

const PLUGIN_DIR: &str = "plugins";
const MAX_EVENT_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_HOST_LOG_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum PluginEvent {
    Init,
    PlayerJoin,
    PlayerLeave,
    ChunkLoad,
    ChunkUnload,
    ConfigReload,
    LanguageChange,
}

impl PluginEvent {
    fn export_name(self) -> &'static str {
        match self {
            Self::Init => "qexed_plugin_init",
            Self::PlayerJoin => "qexed_plugin_player_join",
            Self::PlayerLeave => "qexed_plugin_player_leave",
            Self::ChunkLoad => "qexed_plugin_chunk_load",
            Self::ChunkUnload => "qexed_plugin_chunk_unload",
            Self::ConfigReload => "qexed_plugin_config_reload",
            Self::LanguageChange => "qexed_plugin_language_change",
        }
    }
}

pub struct PluginManager {
    plugins: Mutex<Vec<PluginInstance>>,
}

impl PluginManager {
    pub fn load_default() -> Self {
        Self::load_from_dir(PLUGIN_DIR)
    }

    pub fn load_from_dir(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if let Err(err) = fs::create_dir_all(path) {
            log::warn!("插件目录创建失败: path={}, error={err}", path.display());
            return Self::empty();
        }

        let engine = Engine::default();
        let mut plugins = plugin_files(path)
            .into_iter()
            .filter_map(|file| match PluginInstance::load(&engine, file) {
                Ok(plugin) => Some(plugin),
                Err(err) => {
                    log::warn!("WASM 插件加载失败: {err:#}");
                    None
                }
            })
            .collect::<Vec<_>>();

        plugins.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.name.cmp(&right.name))
        });

        if !plugins.is_empty() {
            let summary = plugins
                .iter()
                .map(|plugin| format!("{}({})", plugin.name, plugin.priority))
                .collect::<Vec<_>>()
                .join(", ");
            log::info!("已加载 WASM 插件: {summary}");
        }

        Self {
            plugins: Mutex::new(plugins),
        }
    }

    pub fn emit_init(&self) {
        self.emit_empty(PluginEvent::Init);
    }

    pub fn emit_player_join(&self, player: &OnlinePlayer) {
        self.emit_json(PluginEvent::PlayerJoin, &player_payload(player));
    }

    pub fn emit_player_leave(&self, player: &OnlinePlayer) {
        self.emit_json(PluginEvent::PlayerLeave, &player_payload(player));
    }

    pub fn emit_chunk_load(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        self.emit_json(
            PluginEvent::ChunkLoad,
            &ChunkPayload {
                dimension,
                chunk_x,
                chunk_z,
            },
        );
    }

    pub fn emit_chunk_unload(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        self.emit_json(
            PluginEvent::ChunkUnload,
            &ChunkPayload {
                dimension,
                chunk_x,
                chunk_z,
            },
        );
    }

    pub fn emit_config_reload(&self, path: &str) {
        self.emit_json(PluginEvent::ConfigReload, &ConfigReloadPayload { path });
    }

    pub fn emit_language_change(&self, language: &str) {
        self.emit_json(PluginEvent::LanguageChange, &LanguagePayload { language });
    }

    fn empty() -> Self {
        Self {
            plugins: Mutex::new(Vec::new()),
        }
    }

    fn emit_empty(&self, event: PluginEvent) {
        self.emit(event, &[]);
    }

    fn emit_json<T: Serialize>(&self, event: PluginEvent, payload: &T) {
        let payload = match serde_json::to_vec(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("插件事件序列化失败: event={event:?}, error={err}");
                return;
            }
        };
        self.emit(event, &payload);
    }

    fn emit(&self, event: PluginEvent, payload: &[u8]) {
        let mut plugins = self.plugins.lock().expect("plugin manager poisoned");
        for plugin in plugins.iter_mut() {
            if let Err(err) = plugin.call_event(event, payload) {
                log::warn!(
                    "WASM 插件事件执行失败: plugin={}, event={event:?}, error={err:#}",
                    plugin.name
                );
            }
        }
    }
}

struct PluginInstance {
    name: String,
    priority: i32,
    store: Store<PluginState>,
    instance: Instance,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    dealloc: Option<TypedFunc<(i32, i32), ()>>,
}

impl PluginInstance {
    fn load(engine: &Engine, path: PathBuf) -> Result<Self> {
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin")
            .to_string();
        let module = Module::from_file(engine, &path)
            .with_context(|| format!("编译插件 {}", path.display()))?;
        let mut store = Store::new(engine, PluginState { name: name.clone() });
        let mut linker = Linker::new(engine);
        linker
            .func_wrap("qexed", "log", host_log)
            .context("注册插件宿主日志 API")?;
        let instance = linker
            .instantiate(&mut store, &module)
            .with_context(|| format!("实例化插件 {}", path.display()))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .with_context(|| format!("插件 {name} 缺少导出 memory"))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "qexed_plugin_alloc")
            .with_context(|| format!("插件 {name} 缺少导出 qexed_plugin_alloc"))?;
        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "qexed_plugin_dealloc")
            .ok();
        let priority = instance
            .get_typed_func::<(), i32>(&mut store, "qexed_plugin_priority")
            .ok()
            .map(|priority| priority.call(&mut store, ()))
            .transpose()
            .with_context(|| format!("读取插件 {name} 优先级失败"))?
            .unwrap_or(0);

        Ok(Self {
            name,
            priority,
            store,
            instance,
            memory,
            alloc,
            dealloc,
        })
    }

    fn call_event(&mut self, event: PluginEvent, payload: &[u8]) -> Result<()> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            anyhow::bail!(
                "插件事件 payload 超过限制: {} > {}",
                payload.len(),
                MAX_EVENT_PAYLOAD_BYTES
            );
        }

        match event {
            PluginEvent::Init => {
                let Some(func) = self
                    .instance
                    .get_typed_func::<(), ()>(&mut self.store, event.export_name())
                    .ok()
                else {
                    return Ok(());
                };
                func.call(&mut self.store, ())?;
            }
            _ => {
                let Some(func) = self
                    .instance
                    .get_typed_func::<(i32, i32), ()>(&mut self.store, event.export_name())
                    .ok()
                else {
                    return Ok(());
                };

                let len = i32::try_from(payload.len()).context("插件事件 payload 长度溢出")?;
                let ptr = self.alloc.call(&mut self.store, len)?;
                let offset = usize::try_from(ptr).context("插件分配器返回负地址")?;
                self.memory.write(&mut self.store, offset, payload)?;
                func.call(&mut self.store, (ptr, len))?;

                if let Some(dealloc) = &self.dealloc {
                    dealloc.call(&mut self.store, (ptr, len))?;
                }
            }
        }
        Ok(())
    }
}

struct PluginState {
    name: String,
}

#[derive(Serialize)]
struct PlayerPayload<'a> {
    uuid: String,
    username: &'a str,
    entity_id: i32,
}

#[derive(Serialize)]
struct ChunkPayload<'a> {
    dimension: &'a str,
    chunk_x: i32,
    chunk_z: i32,
}

#[derive(Serialize)]
struct ConfigReloadPayload<'a> {
    path: &'a str,
}

#[derive(Serialize)]
struct LanguagePayload<'a> {
    language: &'a str,
}

fn player_payload(player: &OnlinePlayer) -> PlayerPayload<'_> {
    PlayerPayload {
        uuid: player.profile.uuid.to_string(),
        username: &player.profile.username,
        entity_id: player.entity_id,
    }
}

fn plugin_files(path: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "wasm")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn host_log(mut caller: Caller<'_, PluginState>, ptr: i32, len: i32) {
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

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::plugin_files;

    #[test]
    fn plugin_files_only_keeps_wasm_files_in_stable_order() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path().join("b.wasm"));
        touch(dir.path().join("a.txt"));
        touch(dir.path().join("a.wasm"));

        let names = plugin_files(dir.path())
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(names, ["a.wasm", "b.wasm"]);
    }

    fn touch(path: impl AsRef<Path>) {
        fs::write(path, []).unwrap();
    }
}
