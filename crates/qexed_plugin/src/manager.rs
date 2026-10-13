//! 宿主侧的插件管理器：插件表、事件总线与各类 HostApi 分发的汇聚点。
//!
//! 管理器在 `init` 完成后以 `&'static` 全局共享，因此插件表用
//! `Mutex<Vec<LoadedPlugin>>`、id 索引用 `RwLock`：加载期独占构建，
//! 运行期事件触发（&self）与关闭期卸载共存。

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Mutex, RwLock};

use qexed_plugin_api::{HostApi, LogApi, LogLevel, Plugin, PluginHandle, RuntimeApi};

use crate::lifecycle::LoadedPlugin;

/// 宿主侧的插件管理器。
pub struct PluginManager {
    /// 已加载插件数组，handle 即下标
    pub(crate) handles: Mutex<Vec<LoadedPlugin>>,
    /// id → handle（冷路径，仅加载/卸载时使用）
    pub(crate) by_id: RwLock<HashMap<&'static str, PluginHandle>>,
    /// 事件处理器 — key = type_name 字符串，RwLock 支持 &self 注册
    pub(crate) event_handlers: RwLock<crate::event::HandlerMap>,
    /// 所有已扫描到的动态库路径（仅 init 期间使用）
    candidates: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(Vec::new()),
            by_id: RwLock::new(HashMap::new()),
            event_handlers: RwLock::new(HashMap::new()),
            candidates: Vec::new(),
        }
    }

    /// 扫描插件目录下的所有 `.dll` / `.so` / `.dylib` 文件。
    pub fn scan_folder(&mut self) -> Result<(), String> {
        self.candidates.clear();

        let plugin_dir = crate::plugins_path()?;

        if !plugin_dir.is_dir() {
            return Ok(());
        }

        let mut paths: Vec<PathBuf> = Vec::new();
        let entries =
            std::fs::read_dir(plugin_dir).map_err(|e| format!("读取插件目录失败: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("遍历插件目录失败: {e}"))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "dll" | "so" | "dylib") {
                    paths.push(path);
                }
            }
        }
        // 稳定顺序：按文件名排序，保证多次启动加载顺序一致（影响依赖解析）。
        paths.sort();
        self.candidates = paths;

        Ok(())
    }

    /// 加载扫描到的所有候选插件，然后依次调用 async on_load。
    pub async fn load_all(&mut self) {
        let paths: Vec<PathBuf> = std::mem::take(&mut self.candidates);
        let mut loaded_handles: Vec<PluginHandle> = Vec::new();

        for path in paths {
            let plugin_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("<?>");
            match crate::lifecycle::load_one(self, &path) {
                Ok(handle) => {
                    loaded_handles.push(handle);
                    let id = self.handles.lock().unwrap()[handle.0].meta.id;
                    self.log(
                        LogLevel::Info,
                        &format!("插件 [{id}] 已加载 (handle={})", handle.0),
                    );
                }
                Err(e) => self.log(
                    LogLevel::Warn,
                    &format!("加载插件 {name} 失败: {e}", name = plugin_name),
                ),
            }
        }

        self.call_on_load(&loaded_handles).await;
    }

    /// 启用所有已加载的插件 —— async。
    pub async fn enable_all(&mut self) {
        let count = self.handles.lock().unwrap().len();
        let handles: Vec<PluginHandle> = (0..count).map(PluginHandle).collect();
        crate::lifecycle::enable_all(self, &handles).await;
    }

    /// 按加载的逆序禁用并卸载所有插件（&self：可在全局静态引用上调用）。
    pub async fn disable_all(&self) {
        loop {
            let Some(handle) = crate::lifecycle::disable_last(self).await else {
                break;
            };
            let _ = handle;
        }
    }

    /// 遍历所有已加载的插件。
    pub fn for_each_plugin(&self, mut f: impl FnMut(PluginHandle, &dyn Plugin)) {
        let handles = self.handles.lock().unwrap();
        for (i, entry) in handles.iter().enumerate() {
            f(PluginHandle(i), entry.plugin.as_ref());
        }
    }

    /// 已加载插件摘要（日志/调试用）。
    pub fn plugin_summaries(&self) -> Vec<String> {
        self.handles
            .lock()
            .unwrap()
            .iter()
            .map(|entry| format!("{} v{}", entry.meta.name, entry.meta.version))
            .collect()
    }

    async fn call_on_load(&mut self, handles: &[PluginHandle]) {
        for &handle in handles {
            let id = self.handles.lock().unwrap()[handle.0].meta.id;
            // SAFETY: host 指向 self，self 在此作用域有效；HostApi 的实现不访问 handles。
            let host: &dyn HostApi = unsafe { &*(self as *mut Self as *const Self) };
            if let Err(e) = self.handles.lock().unwrap()[handle.0].plugin.on_load(host).await {
                self.log(LogLevel::Error, &format!("on_load [{id}] 失败: {e}"));
            }
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

// ——— RuntimeApi ———

impl RuntimeApi for PluginManager {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }
}
