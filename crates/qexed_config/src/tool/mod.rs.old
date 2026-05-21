use serde::{Deserialize, Serialize};
use anyhow::Result;

pub trait AppConfigTrait: Serialize + for<'de> Deserialize<'de> + Default + Sized {
    const PATH: &'static str;
    const NAME: &'static str;

    fn load_or_create_default() -> Result<Self> {
        let path = std::path::Path::new(Self::PATH).join(Self::NAME);
        let path = path.with_extension("toml");

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            
            // 首先尝试直接解析为当前配置结构
            match toml::from_str::<Self>(&content) {
                Ok(config) => {
                    // 检查是否需要更新配置文件（处理新增字段）
                    Self::update_config_file_if_needed(&config, &path, &content)?;
                    Ok(config)
                }
                Err(_) => {
                    // 如果解析失败，可能是由于字段变更，尝试兼容性加载
                    Self::load_with_compatibility(&content, &path)
                }
            }
        } else {
            // 文件不存在，创建默认配置
            let config = Self::default();
            let content = toml::to_string_pretty(&config)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
            Ok(config)
        }
    }

    /// 兼容性加载：处理字段新增、删除、重命名等情况
    fn load_with_compatibility(content: &str, path: &std::path::Path) -> Result<Self> {
        let existing_value: toml::Value = toml::from_str(content)?;
        let default_config = Self::default();
        let default_value = toml::Value::try_from(&default_config)?;
        
        let merged_value = Self::merge_configs(existing_value, default_value);
        
        // 修复：克隆 merged_value 以避免移动后借用错误
        let config: Self = merged_value.clone().try_into()?;
        
        // 如果还需要使用 merged_value，现在它可以继续使用
        // 否则可以直接使用: let config: Self = merged_value.try_into()?;
        
        let updated_content = toml::to_string_pretty(&merged_value)?;
        std::fs::write(path, updated_content)?;
        
        Ok(config)
    }

    /// 深度合并两个配置，优先保留现有配置的值
    fn merge_configs(existing: toml::Value, default: toml::Value) -> toml::Value {
        match (existing, default) {
            // 对于表类型，递归合并每个字段
            (toml::Value::Table(mut existing_table), toml::Value::Table(default_table)) => {
                for (key, default_val) in default_table {
                    if !existing_table.contains_key(&key) {
                        // 现有配置缺少该字段，使用默认值
                        existing_table.insert(key.clone(), default_val);
                    } else {
                        // 字段存在，递归合并子字段（如果是表类型）
                        let existing_val = existing_table[&key].clone();
                        existing_table.insert(key, Self::merge_configs(existing_val, default_val));
                    }
                }
                toml::Value::Table(existing_table)
            }
            // 非表类型或类型不匹配，优先使用现有值
            (existing, _) => existing,
        }
    }

    /// 检查并更新配置文件（处理新增字段等情况）
    fn update_config_file_if_needed(config: &Self, path: &std::path::Path, original_content: &str) -> Result<()> {
        let current_content = toml::to_string_pretty(config)?;
        
        // 将内容解析为 TOML 值进行比较，避免格式差异导致的误判
        let original_value: toml::Value = toml::from_str(original_content)?;
        let current_value = toml::Value::try_from(config)?;
        
        if original_value != current_value {
            // 配置有变化，保存更新
            std::fs::write(path, current_content)?;
        }
        
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let path = std::path::Path::new(Self::PATH).join(Self::NAME);
        let path = path.with_extension("toml");
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// 新增：验证配置完整性
    fn validate(&self) -> Result<()> {
        // 默认实现：总是成功，子类可以重写此方法添加具体验证逻辑
        Ok(())
    }

    /// 新增：重新加载配置（处理外部修改）
    fn reload(&mut self) -> Result<()> {
        let new_config = Self::load_or_create_default()?;
        *self = new_config;
        Ok(())
    }
}