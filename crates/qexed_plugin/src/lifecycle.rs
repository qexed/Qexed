use std::path::Path;

use libloading::{Library, Symbol};
use qexed_plugin_api::{HostApi, LogLevel, Plugin, PluginEntryFn, PluginHandle, PluginMeta};

use crate::manager::PluginManager;

/// 加载和维护插件实例的容器。
pub(crate) struct LoadedPlugin {
    pub(crate) meta: PluginMeta,
    pub(crate) plugin: Box<dyn Plugin>,
    /// 持有动态库句柄，防止被提前释放
    pub(crate) _library: Library,
}

/// 加载单个动态库，Push 到 `mgr.handles`，返回 PluginHandle。
///
/// 不调用 `on_load`，由调用方统一触发。
pub(crate) fn load_one(mgr: &mut PluginManager, path: &Path) -> Result<PluginHandle, String> {
    // SAFETY: 文件已通过 scan_folder 确认存在。
    let library = unsafe { Library::new(path) }.map_err(|e| format!("libloading 加载失败: {e}"))?;

    // SAFETY: 动态库必须导出 __qexed_plugin_create（由 declare_plugin! 保证）。
    let entry: Symbol<PluginEntryFn> = unsafe { library.get(b"__qexed_plugin_create\0") }
        .map_err(|e| format!("找不到入口符号: {e}"))?;

    // SAFETY: 入口函数由 declare_plugin! 生成，返回合法的 Box 指针。
    let plugin: Box<dyn Plugin> = unsafe { Box::from_raw(entry()) };

    let meta = plugin.meta().clone();
    let id = meta.id;

    // 检查依赖
    for dep in meta.dependencies {
        if !mgr.by_id.contains_key(dep) {
            return Err(format!("缺失依赖: {dep}。加载顺序有问题？"));
        }
    }

    let handle = PluginHandle(mgr.handles.len());
    mgr.handles.push(LoadedPlugin {
        meta,
        plugin,
        _library: library,
    });
    mgr.by_id.insert(id, handle);

    Ok(handle)
}

/// 启用一批插件，依次调用 async on_enable。
pub(crate) async fn enable_all(mgr: &mut PluginManager, handles: &[PluginHandle]) {
    let host: *const dyn HostApi = mgr as &dyn HostApi;

    for &handle in handles {
        if handle.0 >= mgr.handles.len() {
            continue;
        }
        let id = mgr.handles[handle.0].meta.id;

        // SAFETY: host 指向 mgr，mgr 在此作用域有效。
        let host = unsafe { &*host };
        if let Err(e) = mgr.handles[handle.0].plugin.on_enable(host).await {
            host.log(
                LogLevel::Error,
                &format!("启用插件 {id} 失败: {e}", id = id, e = e.message),
            );
        }
    }
}

pub(crate) fn disable_one(mgr: &mut PluginManager, handle: PluginHandle) {
    if handle.0 >= mgr.handles.len() {
        return;
    }
    let mut entry = mgr.handles.remove(handle.0);
    let id = entry.meta.id;
    mgr.by_id.remove(id);

    // 移除后，所有后续 handle 的 index 向前移1 —— 更新 by_id
    for (_, h) in mgr.by_id.iter_mut() {
        if h.0 > handle.0 {
            h.0 -= 1;
        }
    }

    if let Err(e) = entry.plugin.on_disable() {
        eprintln!("禁用插件 {id} 时出错: {e}");
    }
    // entry drop → _library drop → 动态库卸载
}
