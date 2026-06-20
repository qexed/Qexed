use qexed_plugin_api::{Plugin, PluginHandle, PluginRefApi};

// ——— PluginRefApi ———

impl PluginRefApi for crate::PluginManager {
    /// 热路径：O(1) 数组下标。
    fn get_plugin(&self, handle: PluginHandle) -> Option<&dyn Plugin> {
        self.handles
            .get(handle.0)
            .map(|entry| entry.plugin.as_ref())
    }

    /// 冷路径：id → handle 查找（仅初始化阶段调用）。
    fn get_plugin_handle(&self, id: &str) -> Option<PluginHandle> {
        self.by_id.get(id).copied()
    }
}
