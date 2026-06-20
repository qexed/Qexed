pub mod event;
pub mod host_api;
pub mod lifecycle;
pub mod manager;

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

pub use manager::PluginManager;

/* ---------- 全局状态 ---------- */

pub static PLUGINS_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static PLUGIN_MANAGER: AtomicPtr<PluginManager> = AtomicPtr::new(std::ptr::null_mut());

/* ---------- 初始化 ---------- */
// 仅执行一次
pub async fn init() -> anyhow::Result<()> {
    if INITIALIZED.load(Ordering::Acquire) != false {
        return Ok(());
    }
    if let Some(path) = PLUGINS_PATH.get() {
        if !path.exists() {
            std::fs::create_dir_all(&path)
                .unwrap_or_else(|e| panic!("无法创建插件目录 {:?}: {}", path, e));
        } else if !path.is_dir() {
            panic!("插件路径 {:?} 存在但不是目录", path);
        }
    } else {
        panic!("PluginPath未定义");
    }

    tklog::info!("加载插件中");
    let mut mgr = PluginManager::new();
    mgr.scan_folder()?;
    mgr.load_all().await;
    mgr.enable_all().await;
    PLUGIN_MANAGER.store(Box::into_raw(Box::new(mgr)), Ordering::Release);
    INITIALIZED.store(true, Ordering::Release);
    tklog::info!("加载插件完成");
    Ok(())
}
pub fn plugin_manager() -> &'static PluginManager {
    unsafe { &*PLUGIN_MANAGER.load(Ordering::Relaxed) }
}
