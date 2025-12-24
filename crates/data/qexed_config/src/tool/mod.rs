use serde::{Deserialize, Serialize};

// Tool 目录旨在简化配置文件管理,而非全量
pub trait AppConfigTrait: Serialize + for<'de> Deserialize<'de> + Default {
    const PATH: &'static str;
    const NAME: &'static str;

    fn load_or_create_default() -> anyhow::Result<Self> {
        // 默认实现：尝试从文件加载，如果失败则创建默认配置并保存
        let path = std::path::Path::new(Self::PATH).join(Self::NAME);
        let path = path.with_extension("toml");

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Self = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Self::default();
            let content = toml::to_string_pretty(&config)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
            Ok(config)
        }
    }

    fn save(&self) -> anyhow::Result<()> {
        let path = std::path::Path::new(Self::PATH).join(Self::NAME);
        let path = path.with_extension("toml");
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }
}
