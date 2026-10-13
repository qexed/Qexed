// ——— LogApi ———
use qexed_plugin_api::{LogApi, LogLevel};

use crate::manager::PluginManager;

impl LogApi for PluginManager {
    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Debug => log::debug!("{message}"),
            LogLevel::Info => log::info!("{message}"),
            LogLevel::Warn => log::warn!("{message}"),
            LogLevel::Error => log::error!("{message}"),
        }
    }
}
