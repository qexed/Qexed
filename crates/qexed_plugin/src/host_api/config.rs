// ——— ConfigApi ———
use qexed_plugin_api::{ConfigApi, PluginError};

use crate::manager::PluginManager;

impl ConfigApi for PluginManager {
    fn read_config_toml(&self, plugin_id: &str) -> Result<Option<String>, PluginError> {
        let config_dir =
            qexed_config::config_path().map_err(|e| PluginError::new(e.to_string()))?;
        let path = config_dir.join(plugin_id).join("config.toml");
        if !path.exists() {
            return Ok(None);
        }
        std::fs::read_to_string(&path)
            .map(Some)
            .map_err(|e| PluginError::new(format!("读取配置失败 [{plugin_id}]: {e}")))
    }

    fn write_config_toml(&self, plugin_id: &str, toml_content: &str) -> Result<(), PluginError> {
        let config_dir =
            qexed_config::config_path().map_err(|e| PluginError::new(e.to_string()))?;
        let dir = config_dir.join(plugin_id);
        std::fs::create_dir_all(&dir)
            .map_err(|e| PluginError::new(format!("创建配置目录失败 [{plugin_id}]: {e}")))?;
        let path = dir.join("config.toml");
        std::fs::write(&path, toml_content)
            .map_err(|e| PluginError::new(format!("写入配置失败 [{plugin_id}]: {e}")))
    }
}
