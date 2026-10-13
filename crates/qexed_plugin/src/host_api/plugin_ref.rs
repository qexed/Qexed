// ——— PluginRefApi ———
use std::sync::Arc;

use qexed_plugin_api::{Plugin, PluginHandle, PluginRefApi};

use crate::manager::PluginManager;

impl PluginRefApi for PluginManager {
    /// 热路径：O(1) 数组下标 + Arc 克隆（锁不外泄）。
    fn get_plugin(&self, handle: PluginHandle) -> Option<Arc<dyn Plugin>> {
        self.handles
            .lock()
            .unwrap()
            .get(handle.0)
            .map(|entry| Arc::clone(&entry.plugin))
    }

    /// 冷路径：id → handle 查找（仅初始化阶段调用）。
    fn get_plugin_handle(&self, id: &str) -> Option<PluginHandle> {
        self.by_id.read().unwrap().get(id).copied()
    }
}
