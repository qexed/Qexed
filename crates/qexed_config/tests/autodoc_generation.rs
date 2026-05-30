use qexed_config::{
    app::{
        qexed::{Qexed, server::World},
        qexed_content_filter::QexedContentFilter,
        qexed_entity::QexedEntity,
        qexed_entity_rendering::QexedEntityRendering,
        qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
        qexed_lan_discovery::QexedLanDiscovery,
        qexed_lobby::QexedLobby,
        qexed_menus::QexedMenus,
        qexed_permissions::QexedPermissions,
        qexed_placeholders::QexedPlaceholders,
        qexed_player_audit::QexedPlayerAudit,
        qexed_player_data::QexedPlayerData,
        qexed_player_messages::QexedPlayerMessages,
        qexed_plugin_download::QexedPluginDownload,
        qexed_resource_pack::QexedResourcePack,
        qexed_scoreboard::QexedScoreboard,
        qexed_server::QexedServer,
        qexed_warden::QexedWarden,
    },
    tool::AppConfigTrait,
};

fn temp_config_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "qexed_config_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间不能早于 UNIX_EPOCH")
            .as_nanos()
    ));
    path
}

fn create_qexed_app_configs(dir: &std::path::Path) -> anyhow::Result<()> {
    let lang = Some("zh-CN".to_string());
    Qexed::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedPluginDownload::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedServer::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedLanDiscovery::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedContentFilter::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedEntity::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedEntityRendering::load_or_create_default(
        lang.clone(),
        Some(true),
        Some(dir.to_path_buf()),
    )?;
    QexedLobby::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedMenus::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedPermissions::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedPlaceholders::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedPlayerAudit::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedPlayerData::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedPlayerMessages::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedResourcePack::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedScoreboard::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    World::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedWarden::load_or_create_default(lang.clone(), Some(true), Some(dir.to_path_buf()))?;
    QexedIpConnectionSpeedTest::load_or_create_default(lang, Some(true), Some(dir.to_path_buf()))?;
    Ok(())
}

#[test]
fn generated_configs_include_autodoc_comments() -> anyhow::Result<()> {
    let dir = temp_config_dir("autodoc");

    create_qexed_app_configs(&dir)?;

    let qexed = std::fs::read_to_string(dir.join("qexed.toml"))?;
    assert!(qexed.contains("# ==== AutoDocHeader ===="));
    assert!(qexed.contains("qexed.toml:"));
    assert!(!qexed.contains("# config.qexed."));
    assert!(!qexed.contains("[server]"));
    assert!(!qexed.contains("[plugin_download]"));

    let plugin_download = std::fs::read_to_string(dir.join("qexed_plugin_download.toml"))?;
    assert!(plugin_download.contains("# ==== AutoDocHeader ===="));
    assert!(plugin_download.contains("qexed_plugin_download.toml:"));
    assert!(plugin_download.contains("[plugin_download]"));
    assert!(plugin_download.contains("插件下载设置"));

    let server = std::fs::read_to_string(dir.join("qexed_server.toml"))?;
    assert!(server.contains("# ==== AutoDocHeader ===="));
    assert!(server.contains("qexed_server.toml:"));
    assert!(server.contains("[server]"));
    assert!(server.contains("ip = \"0.0.0.0:25565\""));
    assert!(!server.contains("[server.world]"));
    assert!(!server.contains("[server.entities]"));

    let world = std::fs::read_to_string(dir.join("world.toml"))?;
    assert!(world.contains("world.toml:"));
    assert!(!world.contains("[server.world]"));
    assert!(world.contains("generator = \"empty\""));

    let warden = std::fs::read_to_string(dir.join("qexed_warden.toml"))?;
    assert!(warden.contains("qexed_warden.toml:"));
    assert!(warden.contains("[data]"));
    assert!(warden.contains("[data.mysql]"));
    assert!(warden.contains("[data.mongodb]"));
    assert!(warden.contains("[data.pika]"));
    assert!(!warden.contains("# config.qexed_warden."));

    let speed_test = std::fs::read_to_string(dir.join("qexed_ip_connect_speed_test.toml"))?;
    assert!(speed_test.contains("qexed_ip_connect_speed_test.toml:"));
    assert!(speed_test.contains("[data]"));
    assert!(speed_test.contains("[data.pika]"));
    assert!(!speed_test.contains("# config.qexed_ip_connection_speed_test."));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_config_keeps_single_autodoc_header() -> anyhow::Result<()> {
    let dir = temp_config_dir("autodoc_header");

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    QexedServer::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let qexed_path = dir.join("qexed.toml");
    let qexed = std::fs::read_to_string(&qexed_path)?;
    let duplicate_header = r#"# ==== AutoDocHeader ====
# 手动制造的重复 Header
# ==== AutoDocHeader ====
# 手动制造的重复 Header
"#;
    std::fs::write(&qexed_path, format!("{duplicate_header}{qexed}"))?;

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let qexed = std::fs::read_to_string(qexed_path)?;
    assert_eq!(qexed.matches("# ==== AutoDocHeader ====").count(), 1);
    let server = std::fs::read_to_string(dir.join("qexed_server.toml"))?;
    assert!(server.contains("服务器监听地址"));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn sensitive_fields_are_written_to_local_secrets_files() -> anyhow::Result<()> {
    let dir = temp_config_dir("sensitive_new");

    let plugin_download = QexedPluginDownload::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;
    let server = QexedServer::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;

    let plugin_download_text = std::fs::read_to_string(dir.join("qexed_plugin_download.toml"))?;
    let server_text = std::fs::read_to_string(dir.join("qexed_server.toml"))?;
    let plugin_secrets =
        std::fs::read_to_string(dir.join(".secrets").join("qexed_plugin_download.toml"))?;
    let server_secrets = std::fs::read_to_string(dir.join(".secrets").join("qexed_server.toml"))?;

    assert!(plugin_download_text.contains("download_token = \"<stored in .secrets>\""));
    assert!(server_text.contains("proxy_token = \"<stored in .secrets>\""));
    assert!(!plugin_download_text.contains(&plugin_download.plugin_download.download_token));
    assert!(!server_text.contains(&server.server.proxy_token));
    assert!(plugin_secrets.contains(&plugin_download.plugin_download.download_token));
    assert!(server_secrets.contains(&server.server.proxy_token));

    let reloaded_plugin = QexedPluginDownload::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;
    let reloaded_server = QexedServer::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;

    assert_eq!(
        reloaded_plugin.plugin_download.download_token,
        plugin_download.plugin_download.download_token
    );
    assert_eq!(
        reloaded_server.server.proxy_token,
        server.server.proxy_token
    );

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_module_configs_get_missing_defaults() -> anyhow::Result<()> {
    let dir = temp_config_dir("nested_defaults");
    std::fs::create_dir_all(&dir)?;

    std::fs::write(
        dir.join("qexed_player_messages.toml"),
        r#"
[player_messages]
enable = false
"#,
    )?;
    std::fs::write(
        dir.join("qexed_content_filter.toml"),
        r#"
[content_filter]
enable = true
"#,
    )?;
    std::fs::write(
        dir.join("qexed_player_data.toml"),
        r#"
[player_data]
enable = true
engine = "vanilla"

[player_data.mongodb]
password = "existing-mongo-password"

[player_data.mysql]
password = "existing-mysql-password"
"#,
    )?;

    let player_messages_config = QexedPlayerMessages::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;
    let content_filter_config = QexedContentFilter::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;
    QexedPermissions::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;
    QexedPlayerData::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;
    let world_config =
        World::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let world = std::fs::read_to_string(dir.join("world.toml"))?;
    let player_data = std::fs::read_to_string(dir.join("qexed_player_data.toml"))?;
    let player_messages = std::fs::read_to_string(dir.join("qexed_player_messages.toml"))?;
    let content_filter = std::fs::read_to_string(dir.join("qexed_content_filter.toml"))?;
    let permissions = std::fs::read_to_string(dir.join("qexed_permissions.toml"))?;
    let secrets = std::fs::read_to_string(dir.join(".secrets").join("qexed_player_data.toml"))?;

    assert!(!player_messages_config.player_messages.enable);
    assert!(content_filter_config.content_filter.enable);
    assert!(!world_config.read_only);
    assert!(world.contains("generator = \"empty\""));
    assert!(world.contains("generator_preset = \"minecraft:classic_flat\""));
    assert!(world.contains("seed = 0"));
    assert!(world.contains("game_mode = \"survival\""));
    assert!(world.contains("spawn_protection_radius = 16"));
    assert!(player_messages.contains("join = \"{player} joined the server\""));
    assert!(player_messages.contains("leave = \"{player} left the server\""));
    assert!(content_filter.contains("engine = \"fixed\""));
    assert!(content_filter.contains("replacement = \"***\""));
    assert!(permissions.contains("[permissions]"));
    assert!(permissions.contains("engine = \"local\""));
    assert!(permissions.contains("local_path = \"config/qexed_permissions.toml\""));
    assert!(permissions.contains("table_prefix = \"luckperms_\""));
    assert!(permissions.contains("allow_by_default = false"));
    assert!(player_data.contains("password = \"<stored in .secrets>\""));
    assert!(secrets.contains("password = \"existing-mongo-password\""));
    assert!(secrets.contains("password = \"existing-mysql-password\""));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn module_configs_are_serialized_as_independent_apps() -> anyhow::Result<()> {
    let dir = temp_config_dir("independent_apps");

    create_qexed_app_configs(&dir)?;

    let qexed = std::fs::read_to_string(dir.join("qexed.toml"))?;
    let server = std::fs::read_to_string(dir.join("qexed_server.toml"))?;
    let entities = std::fs::read_to_string(dir.join("qexed_entity.toml"))?;
    let lobby = std::fs::read_to_string(dir.join("qexed_lobby.toml"))?;
    let menus = std::fs::read_to_string(dir.join("qexed_menus.toml"))?;
    let entity_rendering = std::fs::read_to_string(dir.join("qexed_entity_rendering.toml"))?;

    assert!(!dir.join("qexed.d").join("server.toml").exists());
    assert!(!qexed.contains("[server]"));
    assert!(!qexed.contains("[plugin_download]"));
    assert!(server.contains("[server]"));
    assert!(entities.contains("[entities]"));
    assert!(entities.contains("dimension = \"minecraft:overworld\""));
    assert!(!entities.contains("list = []"));
    assert!(lobby.contains("[lobby]"));
    assert!(menus.contains("[menus]"));
    assert!(entity_rendering.contains("[entity_rendering]"));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn lobby_app_uses_localized_comments() -> anyhow::Result<()> {
    let dir = temp_config_dir("lobby_localized_comment");
    QexedLobby::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let lobby = std::fs::read_to_string(dir.join("qexed_lobby.toml"))?;
    assert!(!lobby.contains("config.qexed.server.lobby"));
    assert!(lobby.contains("AutoDoc"));
    assert!(lobby.contains("大厅"));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn entities_autodoc_does_not_expand_duplicate_blocks_for_array_tables() -> anyhow::Result<()> {
    let dir = temp_config_dir("entities_autodoc_dedup");
    QexedEntity::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let entities_path = dir.join("qexed_entity.toml");
    std::fs::write(
        &entities_path,
        r#"[entities]
enable = true
dimension = "minecraft:overworld"
# ======= AutoDoc =======
# duplicated block 1
# =======================
# ======= AutoDoc =======
# duplicated block 2
# =======================
# ======= AutoDoc =======
# duplicated block 3
# =======================
[[entities.list]]
id = "guide"
kind = "npc"
name = "Guide"
"#,
    )?;

    QexedEntity::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let entities = std::fs::read_to_string(&entities_path)?;
    let marker_count = entities.matches("# ======= AutoDoc =======").count();
    assert!(
        marker_count <= 12,
        "unexpected duplicated AutoDoc markers in qexed_entity.toml: {marker_count}\n{entities}"
    );

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
