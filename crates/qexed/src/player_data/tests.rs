use std::{collections::HashMap, sync::Arc};

use qexed_config::app::qexed::server::{PlayerDataEngine, Spawn};
use qexed_packet::net_types::GameProfile;

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
    assert_eq!(loaded.survival, data.survival);
    assert_eq!(loaded.profile_name, "Alex");
}

#[test]
fn vanilla_playerdata_preserves_unknown_raw_nbt_fields() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("player.dat");
    let profile = GameProfile {
        uuid: uuid::Uuid::new_v4(),
        username: "Alex".to_string(),
        properties: Vec::new(),
    };
    let mut root = HashMap::new();
    root.insert(
        "Dimension".to_string(),
        qexed_nbt::Tag::String(Arc::from("minecraft:overworld")),
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
    qexed_nbt::to_file(&path, "", &qexed_nbt::Tag::Compound(Arc::new(root)), true).unwrap();

    let mut data = read_vanilla_player_data(&path, profile.uuid)
        .unwrap()
        .unwrap();
    data.position.x = 9.0;
    write_vanilla_player_data(&path, &data).unwrap();

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
        store: Arc::new(DisabledPlayerDataStore),
        locks: Arc::new(std::sync::Mutex::new(HashMap::new())),
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
    let inventory = data.inventory();

    assert_eq!(data.survival.health, 20.0);
    assert_eq!(data.survival.food, 20);
    assert_eq!(data.survival.saturation, 5.0);
    assert_eq!(inventory.held_item().item_count.0, 0);
}

#[test]
fn stored_inventory_preserves_main_inventory_slots() {
    let mut inventory = crate::inventory::PlayerInventory::default();
    let changes = inventory
        .add_item_stack(&crate::inventory::simple_item(2, 1))
        .unwrap();
    assert!(matches!(
        &changes[0],
        crate::inventory::InventorySlotChange::Hotbar { slot: 0, .. }
    ));

    for slot in 0..9 {
        inventory
            .add_item_stack(&crate::inventory::simple_item(100 + slot as i32, 64))
            .unwrap();
    }
    let stored = inventory.to_stored();
    let restored = crate::inventory::PlayerInventory::from_stored(&stored);

    assert_eq!(stored.main.len(), 27);
    assert_eq!(restored.to_stored().main, stored.main);
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
