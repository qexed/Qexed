use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::RwLock;

use qexed_plugin_api::{HostApi, LogApi, LogLevel, Plugin, PluginHandle, RuntimeApi};

use crate::lifecycle::LoadedPlugin;

/// 宿主侧的插件管理器。
pub struct PluginManager {
    /// 已加载插件数组，handle 即下标
    pub(crate) handles: Vec<LoadedPlugin>,
    /// id → handle（冷路径，仅加载/卸载时使用）
    pub(crate) by_id: HashMap<&'static str, PluginHandle>,
    /// 事件处理器 — key = TypeId, RwLock 支持 &self 注册
    pub(crate) event_handlers: RwLock<crate::event::HandlerMap>,
    /// 所有已扫描到的动态库路径
    candidates: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
            by_id: HashMap::new(),
            event_handlers: RwLock::new(HashMap::new()),
            candidates: Vec::new(),
        }
    }

    /// 扫描 `PLUGINS_PATH` 下的所有 `.dll` / `.so` / `.dylib` 文件。
    pub fn scan_folder(&mut self) -> Result<(), std::io::Error> {
        self.candidates.clear();

        let plugin_dir = crate::PLUGINS_PATH.get().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "PLUGINS_PATH 未初始化")
        })?;

        if !plugin_dir.is_dir() {
            return Ok(());
        }

        let entries = std::fs::read_dir(plugin_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "dll" | "so" | "dylib") {
                    self.candidates.push(path);
                }
            }
        }

        Ok(())
    }

    /// 加载扫描到的所有候选插件，然后依次调用 async on_load。
    pub async fn load_all(&mut self) {
        let paths: Vec<PathBuf> = self.candidates.drain(..).collect();
        let mut loaded_handles: Vec<PluginHandle> = Vec::new();

        for path in paths {
            let plugin_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("<?>");

            match crate::lifecycle::load_one(self, &path) {
                Ok(handle) => {
                    loaded_handles.push(handle);
                    let id = self.handles[handle.0].meta.id;
                    self.log(
                        LogLevel::Info,
                        &format!("插件 [{id}] 已加载 (handle={h})", h = handle.0),
                    );
                }
                Err(e) => {
                    self.log(
                        LogLevel::Warn,
                        &format!("加载插件 {name} 失败: {e}", name = plugin_name),
                    );
                }
            }
        }

        self.call_on_load(&loaded_handles).await;
    }

    /// 启用所有已加载的插件 —— async。
    pub async fn enable_all(&mut self) {
        let handles: Vec<PluginHandle> = (0..self.handles.len()).map(PluginHandle).collect();
        crate::lifecycle::enable_all(self, &handles).await;
    }

    /// 按加载的逆序禁用并卸载所有插件。
    pub fn disable_all(&mut self) {
        for i in (0..self.handles.len()).rev() {
            crate::lifecycle::disable_one(self, PluginHandle(i));
        }
    }

    /// 遍历所有已加载的插件。
    pub fn for_each_plugin(&self, mut f: impl FnMut(PluginHandle, &dyn Plugin)) {
        for (i, entry) in self.handles.iter().enumerate() {
            f(PluginHandle(i), entry.plugin.as_ref());
        }
    }

    // ——— 内部 ———

    async fn call_on_load(&mut self, handles: &[PluginHandle]) {
        let host: *const dyn HostApi = self as &dyn HostApi;
        for &handle in handles {
            // SAFETY: host 指向 self，self 在此作用域有效；HostApi 方法不访问 handles。
            let host = unsafe { &*host };
            if let Err(e) = self.handles[handle.0].plugin.on_load(host).await {
                let id = self.handles[handle.0].meta.id;
                self.log(LogLevel::Error, &format!("on_load [{id}] 失败: {e}"));
            }
        }
    }
}

// ——— RuntimeApi ———

impl RuntimeApi for PluginManager {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }
}
