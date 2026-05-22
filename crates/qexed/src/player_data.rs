use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use bytes::BytesMut;
use qexed_config::app::qexed::server::{PlayerDataEngine, Spawn};
use qexed_packet::net_types::GameProfile;
use qexed_packet::{PacketCodec, PacketReader, PacketWriter};
use qexed_protocol::to_client::play::{add_entity::EntityPosition, set_equipment::Equipment};
use serde::{Deserialize, Serialize};

use crate::inventory::PlayerInventory;

const DATA_VERSION: i32 = 1;

#[derive(Debug, Clone)]
pub struct PlayerDataManager {
    enabled: bool,
    store: Arc<dyn PlayerDataStore>,
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
}

#[async_trait]
trait PlayerDataStore: Send + Sync + std::fmt::Debug {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>>;
    async fn save(&self, data: &PlayerData) -> Result<()>;
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerData {
    pub data_version: i32,
    pub uuid: uuid::Uuid,
    pub profile_name: String,
    pub dimension: String,
    pub position: StoredPosition,
    pub inventory: StoredInventory,
}

impl PlayerData {
    pub fn from_spawn(profile: &GameProfile, dimension: &str, spawn: &Spawn) -> Self {
        Self {
            data_version: DATA_VERSION,
            uuid: profile.uuid,
            profile_name: profile.username.clone(),
            dimension: dimension.to_string(),
            position: StoredPosition {
                x: spawn.x,
                y: spawn.y,
                z: spawn.z,
                yaw: spawn.yaw,
                pitch: spawn.pitch,
                on_ground: true,
            },
            inventory: PlayerInventory::default().to_stored(),
        }
    }

    pub fn entity_position(&self) -> EntityPosition {
        EntityPosition {
            x: self.position.x,
            y: self.position.y,
            z: self.position.z,
            yaw: self.position.yaw,
            pitch: self.position.pitch,
            on_ground: self.position.on_ground,
        }
    }

    pub fn inventory(&self) -> PlayerInventory {
        PlayerInventory::from_stored(&self.inventory)
    }

    pub fn update_runtime(
        &mut self,
        dimension: &str,
        position: EntityPosition,
        inventory: &PlayerInventory,
    ) {
        self.dimension = dimension.to_string();
        self.position = StoredPosition::from(position);
        self.inventory = inventory.to_stored();
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StoredPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl From<EntityPosition> for StoredPosition {
    fn from(value: EntityPosition) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
            yaw: value.yaw,
            pitch: value.pitch,
            on_ground: value.on_ground,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredInventory {
    pub selected: usize,
    pub hotbar: Vec<StoredSlot>,
    pub equipment: Vec<StoredEquipment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredEquipment {
    pub slot: u8,
    pub item: StoredSlot,
}

impl From<&Equipment> for StoredEquipment {
    fn from(value: &Equipment) -> Self {
        Self {
            slot: value.slot,
            item: StoredSlot::from(&value.item),
        }
    }
}

impl From<&StoredEquipment> for Equipment {
    fn from(value: &StoredEquipment) -> Self {
        Equipment::new(value.slot, (&value.item).into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredSlot {
    #[serde(default)]
    pub packet_data: String,
    #[serde(default, skip_serializing)]
    pub item_count: Option<i32>,
    #[serde(default, skip_serializing)]
    pub item_id: Option<i32>,
}

impl From<&qexed_protocol::types::Slot> for StoredSlot {
    fn from(value: &qexed_protocol::types::Slot) -> Self {
        match encode_slot(value) {
            Ok(packet_data) => Self {
                packet_data,
                item_count: None,
                item_id: None,
            },
            Err(err) => {
                log::warn!("failed to serialize player inventory slot, saving empty slot: {err:#}");
                Self {
                    packet_data: encode_slot(&crate::inventory::empty_slot()).unwrap_or_default(),
                    item_count: None,
                    item_id: None,
                }
            }
        }
    }
}

impl From<&StoredSlot> for qexed_protocol::types::Slot {
    fn from(value: &StoredSlot) -> Self {
        if value.packet_data.is_empty() {
            return legacy_slot(value);
        }

        match decode_slot(&value.packet_data) {
            Ok(slot) => slot,
            Err(err) => {
                log::warn!("failed to decode player inventory slot, loading empty slot: {err:#}");
                crate::inventory::empty_slot()
            }
        }
    }
}

fn legacy_slot(value: &StoredSlot) -> qexed_protocol::types::Slot {
    let item_count = value.item_count.unwrap_or_default();
    if item_count <= 0 {
        return crate::inventory::empty_slot();
    }
    crate::inventory::simple_item(value.item_id.unwrap_or_default(), item_count)
}

fn encode_slot(slot: &qexed_protocol::types::Slot) -> Result<String> {
    let mut bytes = BytesMut::new();
    slot.serialize(&mut PacketWriter::new(&mut bytes))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn decode_slot(value: &str) -> Result<qexed_protocol::types::Slot> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(value)?;
    let mut slice = bytes.as_slice();
    let mut reader = PacketReader::new(&mut slice);
    reader.deserialize()
}

#[derive(Debug)]
struct VanillaPlayerDataStore {
    root: PathBuf,
}

impl VanillaPlayerDataStore {
    fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.root.join(format!("{uuid}.dat"))
    }
}

#[async_trait]
impl PlayerDataStore for VanillaPlayerDataStore {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        let path = self.path(uuid);
        if !path.exists() {
            return Ok(None);
        }
        tokio::task::spawn_blocking(move || read_vanilla_player_data(&path, uuid))
            .await
            .context("join vanilla playerdata read")?
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        tokio::fs::create_dir_all(&self.root)
            .await
            .with_context(|| {
                format!(
                    "create vanilla playerdata directory: {}",
                    self.root.display()
                )
            })?;
        let path = self.path(data.uuid);
        let data = data.clone();
        tokio::task::spawn_blocking(move || write_vanilla_player_data(&path, &data))
            .await
            .context("join vanilla playerdata write")?
    }
}

#[derive(Debug)]
struct MongoPlayerDataStore {
    collection: mongodb::Collection<mongodb::bson::Document>,
}

impl MongoPlayerDataStore {
    async fn new(
        config: &qexed_config::public::mongodb::MongoConfig,
        collection: &str,
    ) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        let client = mongodb::Client::with_uri_str(config.connection_uri()).await?;
        Ok(Self {
            collection: client.database(&config.database).collection(collection),
        })
    }
}

#[async_trait]
impl PlayerDataStore for MongoPlayerDataStore {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        let filter = mongodb::bson::doc! { "_id": uuid.to_string() };
        let Some(doc) = self.collection.find_one(filter).await? else {
            return Ok(None);
        };
        let payload = doc
            .get_str("payload")
            .context("MongoDB player data document missing payload field")?;
        serde_json::from_str(payload)
            .map(Some)
            .context("parse MongoDB player data payload")
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        let payload = serde_json::to_string(data)?;
        let filter = mongodb::bson::doc! { "_id": data.uuid.to_string() };
        let update = mongodb::bson::doc! {
            "$set": {
                "uuid": data.uuid.to_string(),
                "profile_name": &data.profile_name,
                "payload": payload,
                "updated_at": mongodb::bson::DateTime::now(),
            }
        };
        self.collection
            .update_one(filter, update)
            .upsert(true)
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
struct MysqlPlayerDataStore {
    pool: mysql_async::Pool,
    table: String,
}

impl MysqlPlayerDataStore {
    async fn new(config: &qexed_config::public::mysql::MysqlConfig, table: &str) -> Result<Self> {
        config.validate().map_err(anyhow::Error::msg)?;
        validate_mysql_identifier(table)?;
        let opts = mysql_async::Opts::from_url(&config.connection_string())?;
        let pool = mysql_async::Pool::new(opts);
        let store = Self {
            pool,
            table: table.to_string(),
        };
        store.ensure_table().await?;
        Ok(store)
    }

    async fn ensure_table(&self) -> Result<()> {
        use mysql_async::prelude::Queryable;

        let mut conn = self.pool.get_conn().await?;
        conn.query_drop(format!(
            "CREATE TABLE IF NOT EXISTS `{}` (
                `uuid` CHAR(36) NOT NULL PRIMARY KEY,
                `profile_name` VARCHAR(64) NOT NULL,
                `payload` JSON NOT NULL,
                `updated_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )",
            self.table
        ))
        .await?;
        Ok(())
    }
}

#[async_trait]
impl PlayerDataStore for MysqlPlayerDataStore {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let payload: Option<String> = conn
            .exec_first(
                format!(
                    "SELECT `payload` FROM `{}` WHERE `uuid` = :uuid",
                    self.table
                ),
                params! { "uuid" => uuid.to_string() },
            )
            .await?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).context("parse MySQL player data payload")
            })
            .transpose()
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        use mysql_async::{params, prelude::Queryable};

        let payload = serde_json::to_string(data)?;
        let mut conn = self.pool.get_conn().await?;
        conn.exec_drop(
            format!(
                "INSERT INTO `{}` (`uuid`, `profile_name`, `payload`)
                 VALUES (:uuid, :profile_name, :payload)
                 ON DUPLICATE KEY UPDATE
                    `profile_name` = VALUES(`profile_name`),
                    `payload` = VALUES(`payload`)",
                self.table
            ),
            params! {
                "uuid" => data.uuid.to_string(),
                "profile_name" => &data.profile_name,
                "payload" => payload,
            },
        )
        .await?;
        Ok(())
    }
}

fn validate_mysql_identifier(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        anyhow::bail!(
            "MySQL player data table name may only contain ASCII letters, digits, and underscore"
        )
    }
}

fn read_vanilla_player_data(
    path: &std::path::Path,
    uuid: uuid::Uuid,
) -> Result<Option<PlayerData>> {
    let (_, tag) = qexed_nbt::from_file(path)
        .with_context(|| format!("read vanilla playerdata: {}", path.display()))?;
    let qexed = compound(&tag)
        .and_then(|root| root.get("qexed"))
        .and_then(compound)
        .and_then(|qexed| qexed.get("payload"))
        .and_then(string_value);
    if let Some(payload) = qexed {
        return serde_json::from_str(payload)
            .map(Some)
            .context("parse vanilla playerdata qexed payload");
    }

    let root = compound(&tag).context("vanilla playerdata root tag is not compound")?;
    let position = position_from_nbt(root)?;
    let dimension = root
        .get("Dimension")
        .and_then(string_value)
        .unwrap_or("minecraft:overworld")
        .to_string();
    let profile_name = root
        .get("bukkit")
        .and_then(compound)
        .and_then(|bukkit| bukkit.get("lastKnownName"))
        .and_then(string_value)
        .unwrap_or_default()
        .to_string();
    Ok(Some(PlayerData {
        data_version: DATA_VERSION,
        uuid,
        profile_name,
        dimension,
        position,
        inventory: PlayerInventory::default().to_stored(),
    }))
}

fn write_vanilla_player_data(path: &std::path::Path, data: &PlayerData) -> Result<()> {
    let mut root = HashMap::new();
    root.insert("DataVersion".to_string(), qexed_nbt::Tag::Int(DATA_VERSION));
    root.insert(
        "Dimension".to_string(),
        qexed_nbt::Tag::String(Arc::from(data.dimension.clone())),
    );
    root.insert(
        "Pos".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::DOUBLE,
            vec![
                qexed_nbt::Tag::Double(data.position.x),
                qexed_nbt::Tag::Double(data.position.y),
                qexed_nbt::Tag::Double(data.position.z),
            ],
        )?,
    );
    root.insert(
        "Rotation".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::FLOAT,
            vec![
                qexed_nbt::Tag::Float(data.position.yaw),
                qexed_nbt::Tag::Float(data.position.pitch),
            ],
        )?,
    );
    root.insert(
        "OnGround".to_string(),
        qexed_nbt::Tag::Byte(i8::from(data.position.on_ground)),
    );
    root.insert(
        "qexed".to_string(),
        qexed_nbt::Tag::Compound(Arc::new(HashMap::from([(
            "payload".to_string(),
            qexed_nbt::Tag::String(Arc::from(serde_json::to_string(data)?)),
        )]))),
    );

    qexed_nbt::to_file(path, "", &qexed_nbt::Tag::Compound(Arc::new(root)), true)
        .with_context(|| format!("write vanilla playerdata: {}", path.display()))
}

fn position_from_nbt(root: &HashMap<String, qexed_nbt::Tag>) -> Result<StoredPosition> {
    let pos = root
        .get("Pos")
        .and_then(list)
        .context("vanilla playerdata missing Pos")?;
    let rotation = root
        .get("Rotation")
        .and_then(list)
        .context("vanilla playerdata missing Rotation")?;
    Ok(StoredPosition {
        x: double_at(pos, 0)?,
        y: double_at(pos, 1)?,
        z: double_at(pos, 2)?,
        yaw: float_at(rotation, 0)?,
        pitch: float_at(rotation, 1)?,
        on_ground: root
            .get("OnGround")
            .and_then(byte_value)
            .is_none_or(|value| value != 0),
    })
}

fn compound(tag: &qexed_nbt::Tag) -> Option<&HashMap<String, qexed_nbt::Tag>> {
    match tag {
        qexed_nbt::Tag::Compound(value) => Some(value),
        _ => None,
    }
}

fn list(tag: &qexed_nbt::Tag) -> Option<&[qexed_nbt::Tag]> {
    match tag {
        qexed_nbt::Tag::List(_, values) => Some(values),
        _ => None,
    }
}

fn string_value(tag: &qexed_nbt::Tag) -> Option<&str> {
    match tag {
        qexed_nbt::Tag::String(value) => Some(value),
        _ => None,
    }
}

fn byte_value(tag: &qexed_nbt::Tag) -> Option<i8> {
    match tag {
        qexed_nbt::Tag::Byte(value) => Some(*value),
        _ => None,
    }
}

fn double_at(values: &[qexed_nbt::Tag], index: usize) -> Result<f64> {
    match values.get(index) {
        Some(qexed_nbt::Tag::Double(value)) => Ok(*value),
        Some(other) => anyhow::bail!("Pos[{index}] has wrong type: {:?}", other),
        None => anyhow::bail!("Pos missing index {index}"),
    }
}

fn float_at(values: &[qexed_nbt::Tag], index: usize) -> Result<f32> {
    match values.get(index) {
        Some(qexed_nbt::Tag::Float(value)) => Ok(*value),
        Some(other) => anyhow::bail!("Rotation[{index}] has wrong type: {:?}", other),
        None => anyhow::bail!("Rotation missing index {index}"),
    }
}

impl Drop for MysqlPlayerDataStore {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = pool.disconnect().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn vanilla_store_roundtrips_player_data() {
        let temp = tempfile::tempdir().unwrap();
        let config = qexed_config::app::qexed::server::PlayerData {
            engine: PlayerDataEngine::Vanilla,
            ..qexed_config::app::qexed::server::PlayerData::default()
        };
        let manager = PlayerDataManager::from_config(temp.path(), &config)
            .await
            .unwrap();
        let profile = GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Steve".to_string(),
            properties: Vec::new(),
        };
        let mut data = PlayerData::from_spawn(&profile, "minecraft:overworld", &Spawn::default());
        data.position.x = 12.5;

        manager.save(&data).await.unwrap();
        let loaded = manager
            .load_or_default(&profile, "minecraft:overworld", &Spawn::default())
            .await;

        assert_eq!(loaded.position.x, 12.5);
        assert_eq!(loaded.uuid, profile.uuid);
    }

    #[tokio::test]
    async fn disabled_store_uses_default_spawn_without_connecting_backend() {
        let config = qexed_config::app::qexed::server::PlayerData {
            enable: false,
            engine: PlayerDataEngine::Mysql,
            ..qexed_config::app::qexed::server::PlayerData::default()
        };
        let manager = PlayerDataManager::from_config("world", &config)
            .await
            .unwrap();
        let profile = GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Steve".to_string(),
            properties: Vec::new(),
        };
        let spawn = Spawn {
            x: 11.0,
            y: 64.0,
            z: -7.0,
            yaw: 30.0,
            pitch: 10.0,
        };

        let loaded = manager
            .load_or_default(&profile, "minecraft:overworld", &spawn)
            .await;

        assert_eq!(loaded.position.x, 11.0);
        assert_eq!(loaded.position.z, -7.0);
    }

    #[test]
    fn mysql_table_name_rejects_unsafe_identifier() {
        assert!(validate_mysql_identifier("qexed_players_1").is_ok());
        assert!(validate_mysql_identifier("qexed_players;DROP_TABLE").is_err());
        assert!(validate_mysql_identifier("qexed-players").is_err());
    }

    #[test]
    fn stored_slot_preserves_item_components() {
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "qexed_test".to_string(),
            qexed_nbt::Tag::String(Arc::from("kept")),
        );
        let slot = qexed_protocol::types::Slot {
            item_count: qexed_packet::net_types::VarInt(1),
            item_id: Some(qexed_packet::net_types::VarInt(1)),
            number_of_components_to_add: Some(qexed_packet::net_types::VarInt(1)),
            number_of_components_to_remove: Some(qexed_packet::net_types::VarInt(0)),
            components_to_add: Some(vec![
                qexed_protocol::types::ComponentsToAdd::MinecraftCustomData(
                    qexed_protocol::types::minecraft::CustomData {
                        data: qexed_nbt::Tag::Compound(Arc::new(custom_data)),
                    },
                ),
            ]),
            components_to_remove: None,
        };

        let stored = StoredSlot::from(&slot);
        let restored = qexed_protocol::types::Slot::from(&stored);

        assert_eq!(restored, slot);
    }

    #[tokio::test]
    #[ignore = "requires docker/player-data/docker-compose.yml services"]
    async fn mongodb_store_roundtrips_player_data() {
        let config = qexed_config::public::mongodb::MongoConfig {
            username: Some("qexed".to_string()),
            password: Some("qexed".to_string()),
            database: "qexed".to_string(),
            app_name: Some("qexed-test".to_string()),
            auth_source: Some("admin".to_string()),
            ..qexed_config::public::mongodb::MongoConfig::default()
        };
        let store = MongoPlayerDataStore::new(&config, "players_test")
            .await
            .unwrap();
        let data = test_player_data("Mongo");

        store.save(&data).await.unwrap();
        let loaded = store.load(data.uuid).await.unwrap().unwrap();

        assert_eq!(loaded.uuid, data.uuid);
        assert_eq!(loaded.profile_name, "Mongo");
    }

    #[tokio::test]
    #[ignore = "requires docker/player-data/docker-compose.yml services"]
    async fn mysql_store_roundtrips_player_data() {
        let config = qexed_config::public::mysql::MysqlConfig {
            username: "qexed".to_string(),
            password: "qexed".to_string(),
            database: "qexed".to_string(),
            ..qexed_config::public::mysql::MysqlConfig::default()
        };
        let store = MysqlPlayerDataStore::new(&config, "qexed_players_test")
            .await
            .unwrap();
        let data = test_player_data("Mysql");

        store.save(&data).await.unwrap();
        let loaded = store.load(data.uuid).await.unwrap().unwrap();

        assert_eq!(loaded.uuid, data.uuid);
        assert_eq!(loaded.profile_name, "Mysql");
    }

    #[test]
    fn vanilla_playerdata_roundtrips_payload() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("player.dat");
        let profile = GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: "Alex".to_string(),
            properties: Vec::new(),
        };
        let mut data = PlayerData::from_spawn(&profile, "minecraft:overworld", &Spawn::default());
        data.position.yaw = 90.0;

        write_vanilla_player_data(&path, &data).unwrap();
        let loaded = read_vanilla_player_data(&path, profile.uuid)
            .unwrap()
            .unwrap();

        assert_eq!(loaded.position.yaw, 90.0);
        assert_eq!(loaded.profile_name, "Alex");
    }

    fn test_player_data(name: &str) -> PlayerData {
        let profile = GameProfile {
            uuid: uuid::Uuid::new_v4(),
            username: name.to_string(),
            properties: Vec::new(),
        };
        let mut data = PlayerData::from_spawn(&profile, "minecraft:overworld", &Spawn::default());
        data.position.x = 32.0;
        data
    }
}
