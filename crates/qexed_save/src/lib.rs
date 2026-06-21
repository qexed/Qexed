use std::path::{Path, PathBuf};

pub type SaveConfig = qexed_config::app::qexed_save::Save;
pub type StorageEntryConfig = qexed_config::app::qexed_save::StorageEntry;
pub type StorageMode = qexed_config::app::qexed_save::StorageMode;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SaveRoot {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SaveService {
    config: SaveConfig,
    root: SaveRoot,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DimensionId {
    namespace: String,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionKind {
    Chunk,
    Entity,
    Poi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerStoreKind {
    Data,
    Advancements,
    Stats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldStoreKind {
    Data,
    Region,
    Entities,
    Poi,
}

impl SaveService {
    pub fn new(config: SaveConfig) -> anyhow::Result<Self> {
        validate_config(&config)?;
        let root = SaveRoot::from_server_args(&config.root.universe, &config.root.world);
        Ok(Self { config, root })
    }

    pub fn config(&self) -> &SaveConfig {
        &self.config
    }

    pub fn root(&self) -> &SaveRoot {
        &self.root
    }

    pub fn initialize_directories(&self) -> anyhow::Result<()> {
        if !self.config.initialize_directories {
            return Ok(());
        }

        create_dir_if_writable(self.root.path())?;
        create_dir_if_writable(&self.root.path().join(&self.config.player.root_dir))?;
        for kind in [
            PlayerStoreKind::Data,
            PlayerStoreKind::Advancements,
            PlayerStoreKind::Stats,
        ] {
            let store = self.player_store(kind);
            if store.enabled && store.mode.can_write() {
                create_dir_if_writable(&self.player_store_dir(kind))?;
            }
        }

        for dimension in [
            DimensionId::overworld(),
            DimensionId::the_nether(),
            DimensionId::the_end(),
        ] {
            let dimension_root = self.dimension_root(&dimension);
            create_dir_if_writable(&dimension_root)?;
            for kind in [
                WorldStoreKind::Data,
                WorldStoreKind::Region,
                WorldStoreKind::Entities,
                WorldStoreKind::Poi,
            ] {
                let store = self.world_store(kind);
                if store.enabled && store.mode.can_write() {
                    create_dir_if_writable(&self.world_store_dir(&dimension, kind))?;
                }
            }
        }

        Ok(())
    }

    pub fn player_data_path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.player_file_path(PlayerStoreKind::Data, format!("{uuid}.dat"))
    }

    pub fn player_data_file_path(&self, file_name: impl Into<String>) -> PathBuf {
        self.player_file_path(PlayerStoreKind::Data, file_name.into())
    }

    pub fn player_advancements_path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.player_file_path(PlayerStoreKind::Advancements, format!("{uuid}.json"))
    }

    pub fn player_stats_path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.player_file_path(PlayerStoreKind::Stats, format!("{uuid}.json"))
    }

    pub fn dimension_root(&self, dimension: &DimensionId) -> PathBuf {
        self.root
            .path()
            .join(&self.config.world.dimensions_dir)
            .join(dimension.namespace())
            .join(dimension.value())
    }

    pub fn world_store_dir(&self, dimension: &DimensionId, kind: WorldStoreKind) -> PathBuf {
        self.dimension_root(dimension)
            .join(&self.world_store(kind).dir)
    }

    pub fn region_path(
        &self,
        dimension: &DimensionId,
        kind: RegionKind,
        region_x: i32,
        region_z: i32,
    ) -> PathBuf {
        self.world_store_dir(dimension, kind.world_store_kind())
            .join(format!("r.{region_x}.{region_z}.mca"))
    }

    pub fn external_chunk_data_path(
        &self,
        dimension: &DimensionId,
        kind: RegionKind,
        chunk_x: i32,
        chunk_z: i32,
    ) -> PathBuf {
        self.world_store_dir(dimension, kind.world_store_kind())
            .join(format!("c.{chunk_x}.{chunk_z}.mcc"))
    }

    pub fn player_store(&self, kind: PlayerStoreKind) -> &StorageEntryConfig {
        match kind {
            PlayerStoreKind::Data => &self.config.player.data,
            PlayerStoreKind::Advancements => &self.config.player.advancements,
            PlayerStoreKind::Stats => &self.config.player.stats,
        }
    }

    pub fn world_store(&self, kind: WorldStoreKind) -> &StorageEntryConfig {
        match kind {
            WorldStoreKind::Data => &self.config.world.data,
            WorldStoreKind::Region => &self.config.world.region,
            WorldStoreKind::Entities => &self.config.world.entities,
            WorldStoreKind::Poi => &self.config.world.poi,
        }
    }

    fn player_store_dir(&self, kind: PlayerStoreKind) -> PathBuf {
        self.root
            .path()
            .join(&self.config.player.root_dir)
            .join(&self.player_store(kind).dir)
    }

    fn player_file_path(&self, kind: PlayerStoreKind, file_name: String) -> PathBuf {
        self.player_store_dir(kind).join(file_name)
    }
}

impl SaveRoot {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn from_server_args(universe: impl AsRef<Path>, world: impl AsRef<Path>) -> Self {
        Self::new(universe.as_ref().join(world))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn level_dat_path(&self) -> PathBuf {
        self.path.join("level.dat")
    }

    pub fn session_lock_path(&self) -> PathBuf {
        self.path.join("session.lock")
    }

    pub fn player_data_path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.path
            .join("players")
            .join("data")
            .join(format!("{uuid}.dat"))
    }

    pub fn player_advancements_path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.path
            .join("players")
            .join("advancements")
            .join(format!("{uuid}.json"))
    }

    pub fn player_stats_path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.path
            .join("players")
            .join("stats")
            .join(format!("{uuid}.json"))
    }

    pub fn minecraft_data_path(&self, file_name: impl AsRef<Path>) -> PathBuf {
        self.path
            .join("data")
            .join("minecraft")
            .join(file_name.as_ref())
    }

    pub fn map_id_counter_path(&self) -> PathBuf {
        self.path
            .join("data")
            .join("minecraft")
            .join("maps")
            .join("last_id.dat")
    }

    pub fn map_data_path(&self, map_id: i32) -> PathBuf {
        self.path
            .join("data")
            .join("minecraft")
            .join("maps")
            .join(format!("{map_id}.dat"))
    }

    pub fn command_storage_path(&self, namespace: &str) -> PathBuf {
        self.path
            .join("data")
            .join(namespace)
            .join("command_storage.dat")
    }

    pub fn generated_structure_path(&self, namespace: &str, id: &str) -> PathBuf {
        self.path
            .join("generated")
            .join(namespace)
            .join("structure")
            .join(format!("{id}.nbt"))
    }

    pub fn dimension_root(&self, dimension: &DimensionId) -> PathBuf {
        self.path
            .join("dimensions")
            .join(dimension.namespace())
            .join(dimension.value())
    }

    pub fn region_path(
        &self,
        dimension: &DimensionId,
        kind: RegionKind,
        region_x: i32,
        region_z: i32,
    ) -> PathBuf {
        self.dimension_root(dimension)
            .join(kind.directory_name())
            .join(format!("r.{region_x}.{region_z}.mca"))
    }

    pub fn external_chunk_data_path(
        &self,
        dimension: &DimensionId,
        kind: RegionKind,
        chunk_x: i32,
        chunk_z: i32,
    ) -> PathBuf {
        self.dimension_root(dimension)
            .join(kind.directory_name())
            .join(format!("c.{chunk_x}.{chunk_z}.mcc"))
    }
}

impl DimensionId {
    pub fn new(namespace: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            value: value.into(),
        }
    }

    pub fn overworld() -> Self {
        Self::new("minecraft", "overworld")
    }

    pub fn the_nether() -> Self {
        Self::new("minecraft", "the_nether")
    }

    pub fn the_end() -> Self {
        Self::new("minecraft", "the_end")
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl RegionKind {
    fn directory_name(self) -> &'static str {
        match self {
            Self::Chunk => "region",
            Self::Entity => "entities",
            Self::Poi => "poi",
        }
    }

    fn world_store_kind(self) -> WorldStoreKind {
        match self {
            Self::Chunk => WorldStoreKind::Region,
            Self::Entity => WorldStoreKind::Entities,
            Self::Poi => WorldStoreKind::Poi,
        }
    }
}

pub fn region_coordinate(chunk_coordinate: i32) -> i32 {
    chunk_coordinate.div_euclid(32)
}

fn validate_config(config: &SaveConfig) -> anyhow::Result<()> {
    validate_path_segment("player.root_dir", &config.player.root_dir)?;
    validate_path_segment("world.dimensions_dir", &config.world.dimensions_dir)?;

    for (name, store) in [
        ("player.data", &config.player.data),
        ("player.advancements", &config.player.advancements),
        ("player.stats", &config.player.stats),
        ("world.data", &config.world.data),
        ("world.region", &config.world.region),
        ("world.entities", &config.world.entities),
        ("world.poi", &config.world.poi),
    ] {
        validate_path_segment(name, &store.dir)?;
    }

    Ok(())
}

fn validate_path_segment(name: &str, value: &str) -> anyhow::Result<()> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() || value.contains("..") {
        anyhow::bail!("{name} must be a relative directory name without `..`: {value}");
    }
    Ok(())
}

fn create_dir_if_writable(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|err| anyhow::anyhow!("failed to create save directory {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{DimensionId, RegionKind, SaveRoot, SaveService, StorageMode, region_coordinate};

    #[test]
    fn uses_java_26_2_dimension_paths() {
        let root = SaveRoot::new("world");

        assert_eq!(
            root.region_path(&DimensionId::overworld(), RegionKind::Chunk, 0, -1),
            std::path::PathBuf::from("world/dimensions/minecraft/overworld/region/r.0.-1.mca")
        );
        assert_eq!(
            root.region_path(&DimensionId::the_nether(), RegionKind::Entity, 2, 3),
            std::path::PathBuf::from("world/dimensions/minecraft/the_nether/entities/r.2.3.mca")
        );
        assert_eq!(
            root.region_path(&DimensionId::the_end(), RegionKind::Poi, -2, 4),
            std::path::PathBuf::from("world/dimensions/minecraft/the_end/poi/r.-2.4.mca")
        );
    }

    #[test]
    fn uses_players_namespace_after_java_26_1() {
        let root = SaveRoot::new("world");
        let uuid = uuid::Uuid::from_u128(1);

        assert_eq!(
            root.player_data_path(uuid),
            std::path::PathBuf::from("world/players/data/00000000-0000-0000-0000-000000000001.dat")
        );
        assert_eq!(
            root.player_advancements_path(uuid),
            std::path::PathBuf::from(
                "world/players/advancements/00000000-0000-0000-0000-000000000001.json"
            )
        );
        assert_eq!(
            root.player_stats_path(uuid),
            std::path::PathBuf::from(
                "world/players/stats/00000000-0000-0000-0000-000000000001.json"
            )
        );
    }

    #[test]
    fn maps_and_command_storage_are_namespaced() {
        let root = SaveRoot::new("world");

        assert_eq!(
            root.map_id_counter_path(),
            std::path::PathBuf::from("world/data/minecraft/maps/last_id.dat")
        );
        assert_eq!(
            root.map_data_path(5),
            std::path::PathBuf::from("world/data/minecraft/maps/5.dat")
        );
        assert_eq!(
            root.command_storage_path("qexed"),
            std::path::PathBuf::from("world/data/qexed/command_storage.dat")
        );
    }

    #[test]
    fn region_coordinate_uses_floor_division() {
        assert_eq!(region_coordinate(0), 0);
        assert_eq!(region_coordinate(31), 0);
        assert_eq!(region_coordinate(32), 1);
        assert_eq!(region_coordinate(-1), -1);
        assert_eq!(region_coordinate(-32), -1);
        assert_eq!(region_coordinate(-33), -2);
    }

    #[test]
    fn save_service_uses_configured_player_dirs() {
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = "run".to_string();
        config.root.world = "worlds/main".to_string();
        config.player.root_dir = "custom_players".to_string();
        config.player.data.dir = "nbt".to_string();
        config.player.advancements.dir = "adv".to_string();
        config.player.stats.dir = "statistics".to_string();
        let service = SaveService::new(config).unwrap();
        let uuid = uuid::Uuid::from_u128(1);

        assert_eq!(
            service.player_data_path(uuid),
            std::path::PathBuf::from(
                "run/worlds/main/custom_players/nbt/00000000-0000-0000-0000-000000000001.dat"
            )
        );
        assert_eq!(
            service.player_advancements_path(uuid),
            std::path::PathBuf::from(
                "run/worlds/main/custom_players/adv/00000000-0000-0000-0000-000000000001.json"
            )
        );
        assert_eq!(
            service.player_stats_path(uuid),
            std::path::PathBuf::from(
                "run/worlds/main/custom_players/statistics/00000000-0000-0000-0000-000000000001.json"
            )
        );
    }

    #[test]
    fn save_service_uses_configured_world_dirs() {
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.world = "demo".to_string();
        config.world.dimensions_dir = "dims".to_string();
        config.world.region.dir = "chunks".to_string();
        config.world.entities.dir = "mob_storage".to_string();
        let service = SaveService::new(config).unwrap();

        assert_eq!(
            service.region_path(&DimensionId::overworld(), RegionKind::Chunk, 1, 2),
            std::path::PathBuf::from("./demo/dims/minecraft/overworld/chunks/r.1.2.mca")
        );
        assert_eq!(
            service.region_path(&DimensionId::the_nether(), RegionKind::Entity, -1, 0),
            std::path::PathBuf::from("./demo/dims/minecraft/the_nether/mob_storage/r.-1.0.mca")
        );
    }

    #[test]
    fn initializes_writable_directories_only() {
        let temp =
            std::env::temp_dir().join(format!("qexed-save-service-test-{}", std::process::id()));
        if temp.exists() {
            std::fs::remove_dir_all(&temp).unwrap();
        }

        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = temp.to_string_lossy().to_string();
        config.root.world = "world".to_string();
        config.player.advancements.mode = StorageMode::ReadOnly;
        config.world.entities.enabled = false;

        let service = SaveService::new(config).unwrap();
        service.initialize_directories().unwrap();

        let root = temp.join("world");
        assert!(root.join("players/data").is_dir());
        assert!(!root.join("players/advancements").exists());
        assert!(root.join("dimensions/minecraft/overworld/region").is_dir());
        assert!(
            !root
                .join("dimensions/minecraft/overworld/entities")
                .exists()
        );

        std::fs::remove_dir_all(temp).unwrap();
    }
}
