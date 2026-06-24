mod database;
mod model;
mod vanilla;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use qexed_config::app::qexed::server::{PlayerDataEngine, Spawn};
use qexed_packet::net_types::GameProfile;

use database::{MongoPlayerDataStore, MysqlPlayerDataStore};
#[allow(unused_imports)]
pub use model::StoredPosition;
pub(crate) use model::StoredSurvival;
pub use model::{PlayerData, StoredEquipment, StoredInventory, StoredSlot};
use vanilla::VanillaPlayerDataStore;

#[cfg(test)]
use database::validate_mysql_identifier;
#[cfg(test)]
use vanilla::{read_vanilla_player_data, write_vanilla_player_data};

#[derive(Debug, Clone)]
pub struct PlayerDataManager {
    enabled: bool,
    store: Arc<dyn PlayerDataStore>,
    locks: Arc<Mutex<HashMap<uuid::Uuid, Arc<tokio::sync::Mutex<()>>>>>,
}

pub struct PlayerDataLockGuard {
    _local_guard: Option<tokio::sync::OwnedMutexGuard<()>>,
    _store_guard: Option<Box<dyn PlayerDataStoreLock>>,
}

impl PlayerDataManager {
    pub async fn from_config(
        world_path: impl Into<PathBuf>,
        config: &qexed_config::app::qexed::server::PlayerData,
    ) -> Result<Self> {
        if !config.enable {
            return Ok(Self {
                enabled: false,
                store: Arc::new(DisabledPlayerDataStore),
                locks: Arc::new(Mutex::new(HashMap::new())),
            });
        }

        let store: Arc<dyn PlayerDataStore> = match config.engine {
            PlayerDataEngine::Vanilla => {
                let world_path = world_path.into();
                Arc::new(VanillaPlayerDataStore::new(world_path.join("playerdata")))
            }
            PlayerDataEngine::Mongodb => {
                Arc::new(MongoPlayerDataStore::new(&config.mongodb, &config.collection).await?)
            }
            PlayerDataEngine::Mysql => {
                Arc::new(MysqlPlayerDataStore::new(&config.mysql, &config.table).await?)
            }
        };
        Ok(Self {
            enabled: config.enable,
            store,
            locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn lock_player(&self, uuid: uuid::Uuid) -> Result<PlayerDataLockGuard> {
        if !self.enabled {
            return Ok(PlayerDataLockGuard {
                _local_guard: None,
                _store_guard: None,
            });
        }
        let lock = {
            let mut locks = self.locks.lock().expect("player data lock map poisoned");
            locks
                .entry(uuid)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let local_guard = lock.lock_owned().await;
        let store_guard = self.store.lock(uuid).await?;
        Ok(PlayerDataLockGuard {
            _local_guard: Some(local_guard),
            _store_guard: store_guard,
        })
    }

    pub async fn load_or_default(
        &self,
        profile: &GameProfile,
        dimension: &str,
        spawn: &Spawn,
    ) -> PlayerData {
        if self.enabled {
            match self.store.load(profile.uuid).await {
                Ok(Some(mut data)) => {
                    data.profile_name = profile.username.clone();
                    return data;
                }
                Ok(None) => {}
                Err(err) => {
                    log::warn!(
                        "failed to load player data, using default spawn: uuid={}, error={err:#}",
                        profile.uuid
                    );
                }
            }
        }

        PlayerData::from_spawn(profile, dimension, spawn)
    }

    pub async fn save(&self, data: &PlayerData) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        self.store.save(data).await
    }

    pub async fn locked_load_or_default(
        &self,
        _guard: &PlayerDataLockGuard,
        profile: &GameProfile,
        dimension: &str,
        spawn: &Spawn,
    ) -> PlayerData {
        self.load_or_default(profile, dimension, spawn).await
    }

    pub async fn locked_save(&self, _guard: &PlayerDataLockGuard, data: &PlayerData) -> Result<()> {
        self.save(data).await
    }
}

#[async_trait]
trait PlayerDataStore: Send + Sync + std::fmt::Debug {
    async fn lock(&self, _uuid: uuid::Uuid) -> Result<Option<Box<dyn PlayerDataStoreLock>>> {
        Ok(None)
    }

    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>>;
    async fn save(&self, data: &PlayerData) -> Result<()>;
}

trait PlayerDataStoreLock: Send + Sync {}

#[derive(Debug)]
struct DisabledPlayerDataStore;

#[async_trait]
impl PlayerDataStore for DisabledPlayerDataStore {
    async fn load(&self, _uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        Ok(None)
    }

    async fn save(&self, _data: &PlayerData) -> Result<()> {
        Ok(())
    }
}
