use qexed_packet::net_types::GameProfile;

use super::*;

#[tokio::test]
async fn vanilla_store_roundtrips_player_data() {
    let temp = tempfile_dir();
    let config = crate::config::PlayerDataConfig {
        engine: crate::config::PlayerDataEngine::Vanilla,
        ..crate::config::PlayerDataConfig::default()
    };
    let manager = PlayerDataManager::from_config(&temp, &config)
        .await
        .unwrap();
    let profile = GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Steve".to_string(),
        properties: Vec::new(),
    };
    let mut data = PlayerData::from_spawn(&profile, "minecraft:overworld", &Spawn::default());
    data.position.x = 12.5;
    data.survival.health = 17.0;

    manager.save(&data).await.unwrap();
    let loaded = manager
        .load_or_default(&profile, "minecraft:overworld", &Spawn::default())
        .await;

    assert_eq!(loaded.position.x, 12.5);
    assert_eq!(loaded.survival.health, 17.0);
    assert_eq!(loaded.uuid, profile.uuid);
}

#[tokio::test]
async fn disabled_store_uses_default_spawn_without_connecting_backend() {
    let config = crate::config::PlayerDataConfig {
        enable: false,
        engine: crate::config::PlayerDataEngine::Mysql,
        ..crate::config::PlayerDataConfig::default()
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

// TODO(storage): v4 的 mysql_table_name_rejects_unsafe_identifier /
// mongodb_store_roundtrips_player_data / mysql_store_roundtrips_player_data
// 依赖 database.rs 的 mongodb/mysql 实现（v6 未迁入，见 mod.rs 的 TODO(storage)）。

#[test]
fn stored_slot_preserves_item_components() {
    let mut custom_data = std::collections::HashMap::new();
    custom_data.insert(
        "qexed_test".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("kept")),
    );
    let slot = qexed_protocol::types::Slot {
        item_count: qexed_packet::net_types::VarInt(1),
        item_id: Some(qexed_packet::net_types::VarInt(1)),
        number_of_components_to_add: Some(qexed_packet::net_types::VarInt(1)),
        number_of_components_to_remove: Some(qexed_packet::net_types::VarInt(0)),
        components_to_add: Some(vec![
            qexed_protocol::types::ComponentsToAdd::MinecraftCustomData(
                qexed_protocol::types::minecraft::CustomData {
                    data: qexed_nbt::Tag::Compound(std::sync::Arc::new(custom_data)),
                },
            ),
        ]),
        components_to_remove: None,
    };

    let stored = StoredSlot::from(&slot);
    let restored = qexed_protocol::types::Slot::from(&stored);

    assert_eq!(restored, slot);
}

#[test]
fn vanilla_playerdata_roundtrips_payload() {
    let temp = tempfile_dir();
    let path = temp.join("player.dat");
    let profile = GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Alex".to_string(),
        properties: Vec::new(),
    };
    let mut data = PlayerData::from_spawn(&profile, "minecraft:overworld", &Spawn::default());
    data.position.yaw = 90.0;

    vanilla::write_vanilla_player_data(&path, &data).unwrap();
    let loaded = vanilla::read_vanilla_player_data(&path, profile.uuid)
        .unwrap()
        .unwrap();

    assert_eq!(loaded.position.yaw, 90.0);
    assert_eq!(loaded.survival, data.survival);
    assert_eq!(loaded.profile_name, "Alex");
}

#[test]
fn vanilla_playerdata_preserves_unknown_raw_nbt_fields() {
    let temp = tempfile_dir();
    let path = temp.join("player.dat");
    let profile = GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Alex".to_string(),
        properties: Vec::new(),
    };
    let mut root = std::collections::HashMap::new();
    root.insert(
        "Dimension".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from("minecraft:overworld")),
    );
    root.insert(
        "Pos".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::DOUBLE,
            vec![
                qexed_nbt::Tag::Double(1.0),
                qexed_nbt::Tag::Double(2.0),
                qexed_nbt::Tag::Double(3.0),
            ],
        )
        .unwrap(),
    );
    root.insert(
        "Rotation".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::FLOAT,
            vec![qexed_nbt::Tag::Float(0.0), qexed_nbt::Tag::Float(0.0)],
        )
        .unwrap(),
    );
    root.insert("KeepMe".to_string(), qexed_nbt::Tag::Int(42));
    qexed_nbt::to_file(
        &path,
        "",
        &qexed_nbt::Tag::Compound(std::sync::Arc::new(root)),
        true,
    )
    .unwrap();

    let mut data = vanilla::read_vanilla_player_data(&path, profile.uuid)
        .unwrap()
        .unwrap();
    data.position.x = 9.0;
    vanilla::write_vanilla_player_data(&path, &data).unwrap();

    let (_, written) = qexed_nbt::from_file(&path).unwrap();
    let fields = match written {
        qexed_nbt::Tag::Compound(fields) => fields,
        _ => panic!("expected compound playerdata"),
    };
    assert_eq!(fields.get("KeepMe"), Some(&qexed_nbt::Tag::Int(42)));
    assert!(fields.contains_key("qexed"));
}

#[tokio::test]
async fn player_data_lock_serializes_same_uuid() {
    let manager = PlayerDataManager {
        enabled: true,
        store: std::sync::Arc::new(DisabledPlayerDataStore),
        locks: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };
    let uuid = uuid::Uuid::new_v4();
    let first = manager.lock_player(uuid).await;
    let pending = manager.lock_player(uuid);
    tokio::pin!(pending);

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut pending)
            .await
            .is_err()
    );
    drop(first);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), &mut pending)
            .await
            .is_ok()
    );
}

#[test]
fn player_data_defaults_to_empty_survival_inventory() {
    let profile = GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Alex".to_string(),
        properties: Vec::new(),
    };

    let data = PlayerData::from_spawn(&profile, "minecraft:overworld", &Spawn::default());
    let inventory = &data.inventory;

    assert_eq!(data.survival.health, 20.0);
    assert_eq!(data.survival.food, 20);
    assert_eq!(data.survival.saturation, 5.0);
    assert_eq!(inventory.main.len(), 27);
    assert_eq!(inventory.hotbar.len(), 9);
    assert_eq!(inventory.equipment.len(), 6);
    let held = qexed_protocol::types::Slot::from(&inventory.hotbar[inventory.selected]);
    assert_eq!(held.item_count.0, 0);
}

/// tempfile 不在 workspace 依赖里：测试用 uuid 后缀临时目录 + Drop 清理。
fn tempfile_dir() -> std::path::PathBuf {
    TempDir::new()
}

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "qexed_player_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl AsRef<std::path::Path> for TempDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
