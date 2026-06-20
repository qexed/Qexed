// ——— ConfigApi ———
use qexed_plugin_api::{ConfigApi, PluginError};

impl ConfigApi for crate::PluginManager {
    fn read_config_toml(&self, plugin_id: &str) -> Result<Option<String>, PluginError> {
        let config_dir = qexed_config::CONFIG_PATH
            .get()
            .ok_or_else(|| PluginError::new("CONFIG_PATH 未初始化"))?;
        let path = config_dir.join(plugin_id).join("config.toml");
        if !path.exists() {
            return Ok(None);
        }
        std::fs::read_to_string(&path)
            .map(Some)
            .map_err(|e| PluginError::new(format!("读取配置失败 [{plugin_id}]: {e}")))
    }

    fn write_config_toml(&self, plugin_id: &str, toml_content: &str) -> Result<(), PluginError> {
        let config_dir = qexed_config::CONFIG_PATH
            .get()
            .ok_or_else(|| PluginError::new("CONFIG_PATH 未初始化"))?;
        let dir = config_dir.join(plugin_id);
        std::fs::create_dir_all(&dir)
            .map_err(|e| PluginError::new(format!("创建配置目录失败 [{plugin_id}]: {e}")))?;
        let path = dir.join("config.toml");
        std::fs::write(&path, toml_content)
            .map_err(|e| PluginError::new(format!("写入配置失败 [{plugin_id}]: {e}")))
    }
}
