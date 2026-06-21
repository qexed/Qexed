use qexed_packet::net_types::GameProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    profile: GameProfile,
    session: PlayerSession,
    position: PlayerPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSession {
    pub online_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone)]
pub struct PlayerConfigStore {
    save: qexed_save::SaveService,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub version: u32,
    pub uuid: uuid::Uuid,
    pub username: String,
    pub locale: String,
    pub view_distance: i8,
    pub last_position: PlayerPosition,
    pub updated_at_unix_seconds: i64,
}

impl Player {
    pub fn new(profile: GameProfile, session: PlayerSession) -> Self {
        Self {
            profile,
            session,
            position: PlayerPosition::default_spawn(),
        }
    }

    pub fn profile(&self) -> &GameProfile {
        &self.profile
    }

    pub fn uuid(&self) -> uuid::Uuid {
        self.profile.uuid
    }

    pub fn username(&self) -> &str {
        &self.profile.username
    }

    pub fn session(&self) -> &PlayerSession {
        &self.session
    }

    pub fn position(&self) -> PlayerPosition {
        self.position
    }

    pub fn set_position(&mut self, position: PlayerPosition) {
        self.position = position;
    }
}

impl PlayerSession {
    pub fn online() -> Self {
        Self { online_mode: true }
    }

    pub fn offline() -> Self {
        Self { online_mode: false }
    }
}

impl PlayerPosition {
    pub fn default_spawn() -> Self {
        Self {
            x: 0.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

impl PlayerConfig {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn from_player(player: &Player) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            uuid: player.uuid(),
            username: player.username().to_string(),
            locale: "zh_cn".to_string(),
            view_distance: 10,
            last_position: player.position(),
            updated_at_unix_seconds: current_unix_seconds(),
        }
    }
}

impl PlayerConfigStore {
    pub fn new(save: qexed_save::SaveService) -> Self {
        Self { save }
    }

    pub fn path(&self, uuid: uuid::Uuid) -> std::path::PathBuf {
        self.save
            .player_data_file_path(format!("{uuid}.qexed.json"))
    }

    pub fn load(&self, uuid: uuid::Uuid) -> anyhow::Result<Option<PlayerConfig>> {
        let store = self.save.player_store(qexed_save::PlayerStoreKind::Data);
        ensure_read_enabled(store, "player data")?;

        let path = self.path(uuid);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|err| anyhow::anyhow!("读取玩家配置失败 {}: {err}", path.display()))?;
        let config = serde_json::from_str::<PlayerConfig>(&content)
            .map_err(|err| anyhow::anyhow!("玩家配置格式无效 {}: {err}", path.display()))?;
        Ok(Some(config))
    }

    pub fn save(&self, config: &PlayerConfig) -> anyhow::Result<()> {
        let store = self.save.player_store(qexed_save::PlayerStoreKind::Data);
        ensure_write_enabled(store, "player data")?;

        let path = self.path(config.uuid);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                anyhow::anyhow!("创建玩家配置目录失败 {}: {err}", parent.display())
            })?;
        }

        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&path, content)
            .map_err(|err| anyhow::anyhow!("写入玩家配置失败 {}: {err}", path.display()))
    }
}

impl Serialize for PlayerPosition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        [self.x, self.y, self.z, self.yaw as f64, self.pitch as f64].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PlayerPosition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = <[f64; 5]>::deserialize(deserializer)?;
        Ok(Self {
            x: values[0],
            y: values[1],
            z: values[2],
            yaw: values[3] as f32,
            pitch: values[4] as f32,
        })
    }
}

fn ensure_read_enabled(
    store: &qexed_save::StorageEntryConfig,
    name: &'static str,
) -> anyhow::Result<()> {
    if !store.enabled || !store.mode.can_read() {
        anyhow::bail!("{name} 存储未启用读取");
    }
    Ok(())
}

fn ensure_write_enabled(
    store: &qexed_save::StorageEntryConfig,
    name: &'static str,
) -> anyhow::Result<()> {
    if !store.enabled || !store.mode.can_write() {
        anyhow::bail!("{name} 存储未启用写入");
    }
    Ok(())
}

fn current_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{Player, PlayerConfig, PlayerConfigStore, PlayerPosition, PlayerSession};

    #[test]
    fn player_exposes_profile_identity() {
        let profile = qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::from_u128(1),
            username: "Steve".to_string(),
            properties: Vec::new(),
        };
        let player = Player::new(profile, PlayerSession::online());

        assert_eq!(player.uuid(), uuid::Uuid::from_u128(1));
        assert_eq!(player.username(), "Steve");
        assert!(player.session().online_mode);
    }

    #[test]
    fn player_position_can_be_updated() {
        let profile = qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::from_u128(2),
            username: "Alex".to_string(),
            properties: Vec::new(),
        };
        let mut player = Player::new(profile, PlayerSession::offline());
        let position = PlayerPosition {
            x: 1.0,
            y: 65.0,
            z: -1.0,
            yaw: 90.0,
            pitch: 30.0,
        };

        player.set_position(position);

        assert_eq!(player.position(), position);
    }

    #[test]
    fn player_config_round_trips_through_save_service() {
        let temp =
            std::env::temp_dir().join(format!("qexed-player-config-test-{}", std::process::id()));
        if temp.exists() {
            std::fs::remove_dir_all(&temp).unwrap();
        }

        let mut save_config = qexed_save::SaveConfig::default();
        save_config.root.universe = temp.to_string_lossy().to_string();
        save_config.root.world = "world".to_string();
        let save = qexed_save::SaveService::new(save_config).unwrap();
        save.initialize_directories().unwrap();
        let store = PlayerConfigStore::new(save);
        let config = PlayerConfig {
            version: PlayerConfig::CURRENT_VERSION,
            uuid: uuid::Uuid::from_u128(1),
            username: "Steve".to_string(),
            locale: "zh_cn".to_string(),
            view_distance: 12,
            last_position: PlayerPosition {
                x: 1.0,
                y: 65.0,
                z: 2.0,
                yaw: 90.0,
                pitch: 10.0,
            },
            updated_at_unix_seconds: 123,
        };

        store.save(&config).unwrap();
        let loaded = store.load(config.uuid).unwrap().unwrap();

        assert_eq!(loaded, config);
        assert!(
            store
                .path(config.uuid)
                .ends_with("00000000-0000-0000-0000-000000000001.qexed.json")
        );

        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn player_config_respects_read_only_mode() {
        let temp = std::env::temp_dir().join(format!(
            "qexed-player-config-read-only-test-{}",
            std::process::id()
        ));
        if temp.exists() {
            std::fs::remove_dir_all(&temp).unwrap();
        }

        let mut save_config = qexed_save::SaveConfig::default();
        save_config.root.universe = temp.to_string_lossy().to_string();
        save_config.root.world = "world".to_string();
        save_config.player.data.mode = qexed_save::StorageMode::ReadOnly;
        let save = qexed_save::SaveService::new(save_config).unwrap();
        let store = PlayerConfigStore::new(save);
        let profile = qexed_packet::net_types::GameProfile {
            uuid: uuid::Uuid::from_u128(2),
            username: "Alex".to_string(),
            properties: Vec::new(),
        };
        let player = Player::new(profile, PlayerSession::offline());
        let config = PlayerConfig::from_player(&player);

        let err = store.save(&config).unwrap_err();

        assert!(err.to_string().contains("未启用写入"));
        if temp.exists() {
            std::fs::remove_dir_all(temp).unwrap();
        }
    }
}
