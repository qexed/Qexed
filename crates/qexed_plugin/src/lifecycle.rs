//! 动态库加载与插件生命周期状态迁移。

use std::path::Path;
use std::sync::Arc;

use libloading::{Library, Symbol};
use qexed_plugin_api::{HostApi, LogLevel, Plugin, PluginEntryFn, PluginHandle, PluginMeta};

use crate::manager::PluginManager;

/// 加载和维护插件实例的容器。
pub(crate) struct LoadedPlugin {
    pub(crate) meta: PluginMeta,
    pub(crate) plugin: Arc<dyn Plugin>,
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
    let boxed: Box<dyn Plugin> = unsafe { Box::from_raw(entry()) };
    let plugin: Arc<dyn Plugin> = Arc::from(boxed);

    let meta = plugin.meta().clone();
    let id = meta.id;

    // 依赖检查：要求依赖先于自身出现在加载序列中（目录按文件名排序，命名可控制顺序）。
    {
        let by_id = mgr.by_id.read().unwrap();
        for dep in meta.dependencies {
            if !by_id.contains_key(dep) {
                return Err(format!("缺失依赖: {dep}。加载顺序有问题？"));
            }
        }
    }

    let handle = PluginHandle(mgr.handles.lock().unwrap().len());
    mgr.handles.lock().unwrap().push(LoadedPlugin {
        meta,
        plugin,
        _library: library,
    });
    mgr.by_id.write().unwrap().insert(id, handle);

    Ok(handle)
}

/// 启用一批插件，依次调用 async on_enable。
pub(crate) async fn enable_all(mgr: &mut PluginManager, handles: &[PluginHandle]) {
    for &handle in handles {
        let id = match mgr.handles.lock().unwrap().get(handle.0) {
            Some(entry) => entry.meta.id,
            None => continue,
        };

        // SAFETY: host 指向 mgr，mgr 在此作用域有效。
        let host: &dyn HostApi = unsafe { &*(mgr as *mut PluginManager as *const PluginManager) };
        if let Err(e) = mgr.handles.lock().unwrap()[handle.0].plugin.on_enable(host).await {
            host.log(LogLevel::Error, &format!("启用插件 {id} 失败: {e}"));
        }
    }
}

/// 卸载最后一个插件（逆序卸载保证依赖方先于被依赖方关闭）。
///
/// 返回 Some(()) 表示确实卸载了一个插件；空表返回 None。
pub(crate) async fn disable_last(mgr: &PluginManager) -> Option<()> {
    // 事件注册先于库卸载清理：插件持有的 handler 闭包引用其代码段，
    // 卸库后调用 = UB。先摘除该插件的全部 handler，再 drop 动态库。
    let index = {
        let handles = mgr.handles.lock().unwrap();
        if handles.is_empty() {
            return None;
        }
        handles.len() - 1
    };
    crate::event::remove_handlers_of(mgr, PluginHandle(index));

    let entry = {
        let mut handles = mgr.handles.lock().unwrap();
        handles.pop()
    }?;
    let id = entry.meta.id;
    mgr.by_id.write().unwrap().remove(id);
    // 该插件注册的插件命令一并摘除（ActionHub 全局单例存活期 > 插件库）。
    crate::action_hub().command_unregister_plugin(id);

    if let Err(e) = entry.plugin.on_disable() {
        log::warn!("禁用插件 {id} 时出错: {e}");
    }
    // entry drop → _library drop → 动态库卸载
    Some(())
}
