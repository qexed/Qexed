//! qexed 插件宿主：扫描、加载（dll/so/dylib）、生命周期管理与事件分发。
//!
//! # 用法（服务端启动序列）
//!
//! ```ignore
//! qexed_plugin::set_plugins_path("./plugins")?;
//! qexed_plugin::init().await?;   // 扫描 → 加载 → on_load → on_enable → PluginLoadFinishEvent
//! // 运行期：qexed_plugin::plugin_manager().fire(&SomeEvent).await
//! // 关闭前：qexed_plugin::shutdown().await;  // on_disable（逆序）→ 卸库
//! ```

pub mod event;
pub mod host_api;
pub mod lifecycle;
pub mod manager;

use std::path::PathBuf;
use std::sync::OnceLock;

pub use manager::PluginManager;

/* ---------- 全局状态 ---------- */

/// 宿主服务中枢（ActionHub）：玩家动作、NPC 路由、世界编辑、经济、
/// 存储、占位符、跨插件服务的全局单例。与 PLUGIN_MANAGER 生命周期解耦，
/// 惰性初始化（连接循环和插件处理器随时可用）。
static ACTION_HUB: OnceLock<host_api::action::ActionHub> = OnceLock::new();

/// 全局 ActionHub 引用（惰性初始化，永不失败）。
pub fn action_hub() -> &'static host_api::action::ActionHub {
    ACTION_HUB.get_or_init(host_api::action::ActionHub::new)
}

/// 插件目录（dll/so/dylib 所在），由宿主在 init 前设置。
static PLUGINS_PATH: OnceLock<PathBuf> = OnceLock::new();
static PLUGIN_MANAGER: OnceLock<PluginManager> = OnceLock::new();

/// 设置插件目录。必须在 [`init`] 之前调用；重复设置返回错误。
pub fn set_plugins_path(path: impl Into<PathBuf>) -> Result<(), String> {
    PLUGINS_PATH
        .set(path.into())
        .map_err(|_| "PLUGINS_PATH is already initialized".to_string())
}

pub fn plugins_path() -> Result<&'static PathBuf, String> {
    PLUGINS_PATH.get().ok_or_else(|| "PLUGINS_PATH 未初始化".to_string())
}

/// 扫描插件目录、加载全部动态库并依次触发 on_load → on_enable → PluginLoadFinishEvent。
///
/// 幂等：重复调用直接返回 Ok。
pub async fn init() -> Result<(), String> {
    if PLUGIN_MANAGER.get().is_some() {
        return Ok(());
    }
    let path = plugins_path()?;
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("无法创建插件目录 {:?}: {e}", path))?;
    } else if !path.is_dir() {
        return Err(format!("插件路径 {:?} 存在但不是目录", path));
    }

    log::info!("加载插件中");
    let mut mgr = PluginManager::new();
    mgr.scan_folder()?;
    mgr.load_all().await;
    mgr.enable_all().await;
    let result = mgr.fire(&qexed_plugin_api::PluginLoadFinishEvent).await;
    for error in result.errors {
        log::error!("PluginLoadFinishEvent 处理失败: {}", error);
    }
    let _ = PLUGIN_MANAGER.set(mgr);
    log::info!("加载插件完成");
    Ok(())
}

/// 全局插件管理器引用；未初始化时 panic。
pub fn plugin_manager() -> &'static PluginManager {
    try_plugin_manager().expect("qexed_plugin::init 尚未完成")
}

/// 全局插件管理器引用；未初始化时返回 None（连接处理等路径可静默跳过事件触发）。
pub fn try_plugin_manager() -> Option<&'static PluginManager> {
    PLUGIN_MANAGER.get()
}

/// 关闭路径：逆序 on_disable 并释放全部动态库。幂等。
pub async fn shutdown() {
    if let Some(mgr) = PLUGIN_MANAGER.get() {
        mgr.disable_all().await;
        log::info!("全部插件已禁用并卸载");
    }
}
