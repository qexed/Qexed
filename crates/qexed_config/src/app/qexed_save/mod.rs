use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Save {
    pub root: SaveRoot,
    pub initialize_directories: bool,
    pub player: PlayerSave,
    pub world: WorldSave,
}

impl Default for Save {
    fn default() -> Self {
        Self {
            root: SaveRoot::default(),
            initialize_directories: true,
            player: PlayerSave::default(),
            world: WorldSave::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for Save {
    const PATH: &'static str = "/";
    const NAME: &'static str = "save";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRoot {
    pub universe: String,
    pub world: String,
}

impl Default for SaveRoot {
    fn default() -> Self {
        Self {
            universe: ".".to_string(),
            world: "world".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSave {
    pub root_dir: String,
    pub data: StorageEntry,
    pub advancements: StorageEntry,
    pub stats: StorageEntry,
}

impl Default for PlayerSave {
    fn default() -> Self {
        Self {
            root_dir: "players".to_string(),
            data: StorageEntry::new("data"),
            advancements: StorageEntry::new("advancements"),
            stats: StorageEntry::new("stats"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSave {
    pub dimensions_dir: String,
    pub data: StorageEntry,
    pub region: StorageEntry,
    pub entities: StorageEntry,
    pub poi: StorageEntry,
}

impl Default for WorldSave {
    fn default() -> Self {
        Self {
            dimensions_dir: "dimensions".to_string(),
            data: StorageEntry::new("data"),
            region: StorageEntry::new("region"),
            entities: StorageEntry::new("entities"),
            poi: StorageEntry::new("poi"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    pub enabled: bool,
    pub mode: StorageMode,
    pub dir: String,
}

impl StorageEntry {
    fn new(dir: &'static str) -> Self {
        Self {
            enabled: true,
            mode: StorageMode::ReadWrite,
            dir: dir.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageMode {
    ReadWrite,
    ReadOnly,
    Disabled,
}

impl StorageMode {
    pub fn can_read(self) -> bool {
        matches!(self, Self::ReadWrite | Self::ReadOnly)
    }

    pub fn can_write(self) -> bool {
        matches!(self, Self::ReadWrite)
    }
}

#[cfg(test)]
mod tests {
    use super::{Save, StorageMode};
    use qexed_config::tool::AppConfigTrait;

    static CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn creates_save_toml_with_default_storage_layout() {
        let _guard = CONFIG_TEST_LOCK.lock().unwrap();
        let config_dir =
            std::env::temp_dir().join(format!("qexed-save-config-test-{}", std::process::id()));
        let _ = qexed_config::CONFIG_PATH.set(config_dir.clone());
        let config_dir = qexed_config::CONFIG_PATH.get().unwrap().clone();
        let _ = std::fs::remove_dir_all(&config_dir);

        let config = Save::reload_from_disk().unwrap();
        let file = std::fs::read_to_string(config_dir.join("save.toml")).unwrap();

        assert_eq!(config.root.universe, ".");
        assert_eq!(config.root.world, "world");
        assert!(config.initialize_directories);
        assert_eq!(config.player.root_dir, "players");
        assert_eq!(config.player.data.mode, StorageMode::ReadWrite);
        assert_eq!(config.world.dimensions_dir, "dimensions");
        assert_eq!(config.world.region.dir, "region");
        assert!(file.contains("[player]"));
        assert!(file.contains("[world]"));
        assert!(file.contains("mode = \"read_write\""));
    }

    #[test]
    fn reload_from_disk_merges_new_default_save_fields() {
        let _guard = CONFIG_TEST_LOCK.lock().unwrap();
        let config_dir =
            std::env::temp_dir().join(format!("qexed-save-reload-test-{}", std::process::id()));
        let _ = qexed_config::CONFIG_PATH.set(config_dir.clone());
        let config_dir = qexed_config::CONFIG_PATH.get().unwrap().clone();
        let _ = std::fs::remove_dir_all(&config_dir);
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("save.toml"),
            r#"
[root]
universe = "./run"
world = "demo"
"#,
        )
        .unwrap();

        let config = Save::reload_from_disk().unwrap();
        let file = std::fs::read_to_string(config_dir.join("save.toml")).unwrap();

        assert_eq!(config.root.universe, "./run");
        assert_eq!(config.root.world, "demo");
        assert_eq!(config.player.root_dir, "players");
        assert_eq!(config.world.region.dir, "region");
        assert_eq!(config.world.poi.mode, StorageMode::ReadWrite);
        assert!(file.contains("initialize_directories = true"));
        assert!(file.contains("[player.data]"));
        assert!(file.contains("[world.poi]"));

        std::fs::remove_dir_all(config_dir).unwrap();
    }
}
