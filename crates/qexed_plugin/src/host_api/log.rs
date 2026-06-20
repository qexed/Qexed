// ——— LogApi ———
use qexed_plugin_api::{LogApi, LogLevel};

use crate::manager::PluginManager;

impl LogApi for PluginManager {
    fn log(&self, level: LogLevel, message: &str) {
        let msg = message;
        match level {
            LogLevel::Debug => tklog::debug!(msg),
            LogLevel::Info => tklog::info!(msg),
            LogLevel::Warn => tklog::warn!(msg),
            LogLevel::Error => tklog::error!(msg),
        }
    }
}
