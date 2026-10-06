mod model;
mod vanilla;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use qexed_packet::net_types::GameProfile;

use crate::error::Result;
use vanilla::VanillaPlayerDataStore;

pub use model::{PlayerData, StoredEquipment, StoredInventory, StoredPosition, StoredSlot};

/// 出生点（v4 qexed_config::app::qexed::server::Spawn 的 v6 替代；
/// 配置体系迁入 qexed_config 后改由那边提供，这里保留同构结构）。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

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
    /// 从配置构造（v6：vanilla 引擎用 world_path；mongodb/mysql 见 TODO(storage)）。
    pub async fn from_config(
        world_path: impl Into<PathBuf>,
        config: &crate::config::PlayerDataConfig,
    ) -> Result<Self> {
        if !config.enable {
            return Ok(Self {
                enabled: false,
                store: Arc::new(DisabledPlayerDataStore),
                locks: Arc::new(Mutex::new(HashMap::new())),
            });
        }

        let store: Arc<dyn PlayerDataStore> = match config.engine {
            crate::config::PlayerDataEngine::Vanilla => {
                let world_path = world_path.into();
                Arc::new(VanillaPlayerDataStore::new(world_path.join("playerdata")))
            }
            // TODO(storage): v6 workspace 无 mongodb/mysql 依赖。
            // Mongodb 引擎迁移方案：MongoPlayerDataStore::new(&config.mongodb, &config.collection)
            // —— 依赖 qexed_config 补 MongoConfig + mongodb crate 进 workspace 后在
            //    player_data/database.rs 实现 PlayerDataStore trait（v4 代码见 qexed-v4）。
            crate::config::PlayerDataEngine::Mongodb => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.player.data.storage_unavailable")
                        .replace("%{engine}", "mongodb")
                );
                Arc::new(DisabledPlayerDataStore)
            }
            // TODO(storage): v6 workspace 无 mongodb/mysql 依赖。
            // Mysql 引擎迁移方案：MysqlPlayerDataStore::new(&config.mysql, &config.table)
            // —— 同上，依赖 qexed_config 补 MysqlConfig + mysql_async crate 进 workspace。
            crate::config::PlayerDataEngine::Mysql => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.player.data.storage_unavailable")
                        .replace("%{engine}", "mysql")
                );
                Arc::new(DisabledPlayerDataStore)
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
                        "{}",
                        qexed_language::t("qexed.player.data.load_failed")
                            .replace("%{uuid}", &profile.uuid.to_string())
                            .replace("%{error}", &format!("{err:#}"))
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

/// 玩家数据存储后端接口。
///
/// # TODO(storage)
/// v4 的 mongodb / mysql 实现（database.rs）依赖 v6 workspace 尚未引入的
/// mongodb / mysql_async crate，暂不迁移；恢复时在此 trait 上实现：
/// - MongoPlayerDataStore：update_one upsert + payload JSON + raw_nbt Binary + 租约锁
/// - MysqlPlayerDataStore：GET_LOCK/RELEASE_LOCK + upsert SQL
/// v4 参考实现：qexed-v4/crates/qexed/src/player_data/database.rs
/// 玩家数据存储后端接口。
///
/// # TODO(storage)
/// v4 的 mongodb / mysql 实现（database.rs）依赖 v6 workspace 尚未引入的
/// mongodb / mysql_async crate，暂不迁移；恢复时在此 trait 上实现：
/// - MongoPlayerDataStore：update_one upsert + payload JSON + raw_nbt Binary + 租约锁
/// - MysqlPlayerDataStore：GET_LOCK/RELEASE_LOCK + upsert SQL
/// v4 参考实现：qexed-v4/crates/qexed/src/player_data/database.rs
pub trait PlayerDataStore: Send + Sync + std::fmt::Debug {
    fn lock(
        &self,
        _uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<Box<dyn PlayerDataStoreLock>>>> + Send + '_>>
    {
        Box::pin(async { Ok(None) })
    }

    fn load(
        &self,
        uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<PlayerData>>> + Send + '_>>;

    fn save(
        &self,
        data: &PlayerData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;
}

pub trait PlayerDataStoreLock: Send + Sync {}

#[derive(Debug)]
struct DisabledPlayerDataStore;

impl PlayerDataStore for DisabledPlayerDataStore {
    fn load(
        &self,
        _uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<PlayerData>>> + Send + '_>> {
        Box::pin(async { Ok(None) })
    }

    fn save(
        &self,
        _data: &PlayerData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}