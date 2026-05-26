use qexed_config::{
    app::{
        qexed::{
            Qexed,
            server::{ContentFilterEngine, GpuDeviceSelector},
        },
        qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
        qexed_warden::QexedWarden,
    },
    tool::AppConfigTrait,
};
use toml_edit::DocumentMut;

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

fn load_split_qexed_config(dir: &std::path::Path) -> anyhow::Result<Qexed> {
    let qexed_path = dir.join("qexed.toml");
    let mut doc = std::fs::read_to_string(&qexed_path)?.parse::<DocumentMut>()?;
    let split_dir = dir.join("qexed.d");

    for file_name in [
        "plugin_download.toml",
        "server.toml",
        "world.toml",
        "lan_discovery.toml",
        "player_data.toml",
        "player_messages.toml",
        "content_filter.toml",
        "permissions.toml",
        "resource_pack.toml",
        "entities.toml",
        "scoreboard.toml",
    ] {
        let path = split_dir.join(file_name);
        if path.exists() {
            let split_doc = std::fs::read_to_string(path)?.parse::<DocumentMut>()?;
            merge_toml_documents(doc.as_item_mut(), split_doc.as_item());
        }
    }

    let mut clean = DocumentMut::new();
    for (key, item) in doc.iter() {
        if !key.starts_with("auto_doc_") {
            clean.insert(key, item.clone());
        }
    }

    Ok(toml::from_str(&clean.to_string())?)
}

fn merge_toml_documents(target: &mut toml_edit::Item, source: &toml_edit::Item) {
    match (target, source) {
        (toml_edit::Item::Table(target_table), toml_edit::Item::Table(source_table)) => {
            for (key, source_item) in source_table.iter() {
                if key.starts_with("auto_doc_") {
                    continue;
                }

                match target_table.get_mut(key) {
                    Some(target_item) => merge_toml_documents(target_item, source_item),
                    None => {
                        target_table.insert(key, source_item.clone());
                    }
                }
            }
        }
        (target_item, source_item) => {
            *target_item = source_item.clone();
        }
    }
}

#[test]
fn generated_configs_include_autodoc_comments() -> anyhow::Result<()> {
    let dir = temp_config_dir("autodoc");

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    QexedWarden::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    QexedIpConnectionSpeedTest::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;

    let qexed = std::fs::read_to_string(dir.join("qexed.toml"))?;
    assert!(qexed.contains("# ==== AutoDocHeader ===="));
    assert!(!qexed.contains("# config.qexed."));

    let plugin_download =
        std::fs::read_to_string(dir.join("qexed.d").join("plugin_download.toml"))?;
    assert!(plugin_download.contains("插件下载设置"));

    let server = std::fs::read_to_string(dir.join("qexed.d").join("server.toml"))?;
    assert!(server.contains("[server]"));
    assert!(server.contains("ip = \"0.0.0.0:25565\""));

    let world = std::fs::read_to_string(dir.join("qexed.d").join("world.toml"))?;
    assert!(world.contains("[server.world]"));
    assert!(world.contains("generator = \"empty\""));

    let warden = std::fs::read_to_string(dir.join("qexed_warden.toml"))?;
    assert!(warden.contains("典狱长数据存储设置"));
    assert!(warden.contains("MySQL 服务器地址"));
    assert!(warden.contains("MongoDB 服务器地址"));
    assert!(warden.contains("Pika 服务器地址"));
    assert!(!warden.contains("# config.qexed_warden."));

    let speed_test = std::fs::read_to_string(dir.join("qexed_ip_connect_speed_test.toml"))?;
    assert!(speed_test.contains("IP 连接速率检测"));
    assert!(speed_test.contains("Pika 服务器地址"));
    assert!(!speed_test.contains("# config.qexed_ip_connection_speed_test."));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_config_keeps_single_autodoc_header() -> anyhow::Result<()> {
    let dir = temp_config_dir("autodoc_header");

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

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
    let server = std::fs::read_to_string(dir.join("qexed.d").join("server.toml"))?;
    assert!(server.contains("服务器监听地址"));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn sensitive_fields_are_written_to_local_secrets_file() -> anyhow::Result<()> {
    let dir = temp_config_dir("sensitive_new");

    let config =
        Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    let qexed = std::fs::read_to_string(dir.join("qexed.toml"))?;
    let plugin_download =
        std::fs::read_to_string(dir.join("qexed.d").join("plugin_download.toml"))?;
    let server = std::fs::read_to_string(dir.join("qexed.d").join("server.toml"))?;
    let secrets = std::fs::read_to_string(dir.join(".secrets").join("qexed.toml"))?;

    assert!(plugin_download.contains("download_token = \"<stored in .secrets>\""));
    assert!(server.contains("proxy_token = \"<stored in .secrets>\""));
    assert!(!qexed.contains(&config.plugin_download.download_token));
    assert!(!qexed.contains(&config.server.proxy_token));
    assert!(secrets.contains(&config.plugin_download.download_token));
    assert!(secrets.contains(&config.server.proxy_token));

    let reloaded =
        Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    let secrets_after_reload = std::fs::read_to_string(dir.join(".secrets").join("qexed.toml"))?;

    assert_eq!(
        reloaded.plugin_download.download_token,
        config.plugin_download.download_token
    );
    assert_eq!(reloaded.server.proxy_token, config.server.proxy_token);
    assert!(!secrets_after_reload.contains("<stored in .secrets>"));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_sensitive_fields_are_migrated_to_local_secrets_file() -> anyhow::Result<()> {
    let dir = temp_config_dir("sensitive_migration");
    let qexed_path = dir.join("qexed.toml");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        &qexed_path,
        r#"
version = 0
update_check = true
language = "zh-CN"

[plugin_download]
enable = false
download = "https://api.example.com/plugins/"
download_token = "existing-token"

[server]
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "existing-proxy-token"
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[server.world]
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[server.world.spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
    )?;

    let config =
        Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    let qexed = std::fs::read_to_string(&qexed_path)?;
    let plugin_download =
        std::fs::read_to_string(dir.join("qexed.d").join("plugin_download.toml"))?;
    let server = std::fs::read_to_string(dir.join("qexed.d").join("server.toml"))?;
    let secrets = std::fs::read_to_string(dir.join(".secrets").join("qexed.toml"))?;

    assert_eq!(config.plugin_download.download_token, "existing-token");
    assert_eq!(config.server.proxy_token, "existing-proxy-token");
    assert!(plugin_download.contains("download_token = \"<stored in .secrets>\""));
    assert!(server.contains("proxy_token = \"<stored in .secrets>\""));
    assert!(!qexed.contains("existing-token"));
    assert!(!qexed.contains("existing-proxy-token"));
    assert!(secrets.contains("download_token = \"existing-token\""));
    assert!(secrets.contains("proxy_token = \"existing-proxy-token\""));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_config_gets_missing_nested_defaults() -> anyhow::Result<()> {
    let dir = temp_config_dir("nested_defaults");
    let qexed_path = dir.join("qexed.toml");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        &qexed_path,
        r#"
version = 0
update_check = true
language = "zh-CN"

[plugin_download]
enable = false
download = "https://api.example.com/plugins/"
download_token = "existing-token"

[server]
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "existing-proxy-token"
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[server.world]
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
light_algorithm = "fast"

[server.world.spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
    )?;

    let config =
        Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    let qexed = std::fs::read_to_string(&qexed_path)?;
    let world = std::fs::read_to_string(dir.join("qexed.d").join("world.toml"))?;
    let player_data = std::fs::read_to_string(dir.join("qexed.d").join("player_data.toml"))?;
    let player_messages =
        std::fs::read_to_string(dir.join("qexed.d").join("player_messages.toml"))?;
    let content_filter = std::fs::read_to_string(dir.join("qexed.d").join("content_filter.toml"))?;
    let permissions = std::fs::read_to_string(dir.join("qexed.d").join("permissions.toml"))?;

    assert!(!config.server.world.read_only);
    assert!(qexed.contains("# ==== AutoDocHeader ===="));
    assert!(world.contains("generator = \"empty\""));
    assert!(world.contains("generator_preset = \"minecraft:classic_flat\""));
    assert!(world.contains("seed = 0"));
    assert!(world.contains("game_mode = \"survival\""));
    assert!(world.contains("spawn_protection_radius = 16"));
    assert!(player_data.contains("[server.player_data]"));
    assert!(player_messages.contains("[server.player_messages]"));
    assert!(player_messages.contains("join = \"{player} joined the server\""));
    assert!(content_filter.contains("[server.content_filter]"));
    assert!(permissions.contains("[server.permissions]"));
    assert!(permissions.contains("engine = \"local\""));
    assert!(permissions.contains("local_path = \"config/qexed_permissions.toml\""));
    assert!(permissions.contains("table_prefix = \"luckperms_\""));
    assert!(content_filter.contains("replacement = \"***\""));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn deserializable_config_still_gets_missing_serde_default_fields() -> anyhow::Result<()> {
    let dir = temp_config_dir("serde_defaults");
    let qexed_path = dir.join("qexed.toml");
    std::fs::create_dir_all(&dir)?;
    let original = r#"
version = 0
update_check = true
language = "zh-CN"

[plugin_download]
enable = false
download = "https://api.example.com/plugins/"
download_token = "existing-token"

[server]
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "existing-proxy-token"
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[server.world]
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"
light_algorithm = "fast"
gpu = { enable = true }

[server.world.spawn]
x = 0.0
y = 64.0
z = 0.0
yaw = 0.0
pitch = 0.0

[server.player_messages]
enable = false

[server.content_filter]
enable = true
"#;
    std::fs::write(&qexed_path, original)?;

    assert!(toml::from_str::<Qexed>(original).is_ok());
    assert!(!original.contains("join ="));
    assert!(!original.contains("engine ="));
    assert!(!original.contains("device ="));

    let config =
        Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    let qexed = std::fs::read_to_string(&qexed_path)?;
    let saved_config = load_split_qexed_config(&dir)?;
    let world = std::fs::read_to_string(dir.join("qexed.d").join("world.toml"))?;
    let entities = std::fs::read_to_string(dir.join("qexed.d").join("entities.toml"))?;
    let player_messages =
        std::fs::read_to_string(dir.join("qexed.d").join("player_messages.toml"))?;
    let content_filter = std::fs::read_to_string(dir.join("qexed.d").join("content_filter.toml"))?;
    let permissions = std::fs::read_to_string(dir.join("qexed.d").join("permissions.toml"))?;

    assert!(!config.server.player_messages.enable);
    assert!(config.server.content_filter.enable);
    assert!(saved_config.server.world.gpu.enable);
    assert_eq!(
        saved_config.server.world.gpu.device,
        GpuDeviceSelector::Discrete
    );
    assert!(qexed.contains("# ==== AutoDocHeader ===="));
    assert!(world.contains("generator = \"empty\""));
    assert!(world.contains("generator_preset = \"minecraft:classic_flat\""));
    assert!(world.contains("seed = 0"));
    assert!(entities.contains("[server.entities]"));
    assert!(entities.contains("dimension = \"minecraft:overworld\""));
    assert!(!entities.contains("list = []"));
    assert_eq!(
        saved_config.server.content_filter.engine,
        ContentFilterEngine::Fixed
    );
    assert!(player_messages.contains("enable = false"));
    assert!(player_messages.contains("join = \"{player} joined the server\""));
    assert!(player_messages.contains("leave = \"{player} left the server\""));
    assert!(content_filter.contains("engine = \"fixed\""));
    assert!(content_filter.contains("replacement = \"***\""));
    assert!(
        content_filter
            .contains("block_message = \"Your message was blocked by the server content filter.\"")
    );
    assert!(permissions.contains("[server.permissions]"));
    assert!(permissions.contains("engine = \"local\""));
    assert!(permissions.contains("allow_by_default = true"));
    assert!(world.contains("device = \"discrete\""));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_config_with_later_nested_tables_gets_new_sibling_defaults() -> anyhow::Result<()> {
    let dir = temp_config_dir("sibling_defaults");
    let qexed_path = dir.join("qexed.toml");
    std::fs::create_dir_all(&dir)?;
    let original = r#"
version = 0
update_check = true
language = "zh-CN"

[plugin_download]
enable = false
download = "https://api.example.com/plugins/"
download_token = "existing-token"

[server]
ip = "0.0.0.0:25565"
online = false
max_player = -1
display_players = true
online_mode = false
network_compression_threshold = 256
proxy = false
proxy_protocol = "QTunnel"
proxy_token = "existing-proxy-token"
max_port_connections = 65535
rate_limit_window_secs = 60
rate_limit_max_attempts = 6
motd = ["Welcome"]
code_of_conduct = false
favicon = ""

[server.lan_discovery]
enable = true
interval_ms = 1500

[server.world]
path = "world"
read_only = false
game_mode = "survival"
spawn_protection_radius = 16
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 10
chunk_load_parallelism = 64
simulation_distance = 10
light = "static"
light_algorithm = "ray_trace"

[server.world.gpu]
enable = true
device = "discrete"

[server.world.spawn]
x = 0.0
y = 333.0
z = 0.0
yaw = 0.0
pitch = 0.0

[server.player_data]
enable = true
engine = "vanilla"
collection = "players"
table = "qexed_players"

[server.player_data.mongodb]
host = "127.0.0.1"
port = 27017
username = "qexed"
password = "existing-mongo-password"
database = "qexed"
app_name = "qexed"
auth_source = "admin"
use_tls = false
connect_timeout = "10s"
socket_timeout = "5s"
max_pool_size = 100
min_pool_size = 0
max_idle_time = "60s"

[server.player_data.mysql]
ip = "127.0.0.1"
port = 3306
username = "qexed"
password = "existing-mysql-password"
database = "qexed"
pool_max_size = 10
pool_min_idle = 2
connection_timeout = "30s"
idle_timeout = "300s"
use_ssl = false
charset = "utf8mb4"
options = []
"#;
    std::fs::write(&qexed_path, original)?;

    assert!(toml::from_str::<Qexed>(original).is_ok());
    assert!(!original.contains("[server.player_messages]"));
    assert!(!original.contains("[server.content_filter]"));
    assert!(!original.contains("[server.permissions]"));
    assert!(!original.contains("generator ="));

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    let qexed = std::fs::read_to_string(&qexed_path)?;
    let world = std::fs::read_to_string(dir.join("qexed.d").join("world.toml"))?;
    let player_messages =
        std::fs::read_to_string(dir.join("qexed.d").join("player_messages.toml"))?;
    let content_filter = std::fs::read_to_string(dir.join("qexed.d").join("content_filter.toml"))?;
    let permissions = std::fs::read_to_string(dir.join("qexed.d").join("permissions.toml"))?;
    let player_data = std::fs::read_to_string(dir.join("qexed.d").join("player_data.toml"))?;
    let secrets = std::fs::read_to_string(dir.join(".secrets").join("qexed.toml"))?;

    assert!(qexed.contains("# ==== AutoDocHeader ===="));
    assert!(player_messages.contains("[server.player_messages]"));
    assert!(world.contains("generator = \"empty\""));
    assert!(world.contains("generator_preset = \"minecraft:classic_flat\""));
    assert!(world.contains("seed = 0"));
    assert!(player_messages.contains("join = \"{player} joined the server\""));
    assert!(content_filter.contains("[server.content_filter]"));
    assert!(permissions.contains("[server.permissions]"));
    assert!(content_filter.contains("engine = \"fixed\""));
    assert!(permissions.contains("engine = \"local\""));
    assert!(!qexed.contains("existing-mongo-password"));
    assert!(!qexed.contains("existing-mysql-password"));
    assert!(player_data.contains("password = \"<stored in .secrets>\""));
    assert!(secrets.contains("password = \"existing-mongo-password\""));
    assert!(secrets.contains("password = \"existing-mysql-password\""));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn split_qexed_config_files_override_main_config() -> anyhow::Result<()> {
    let dir = temp_config_dir("split_override");
    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let qexed_path = dir.join("qexed.toml");
    let split_dir = dir.join("qexed.d");
    std::fs::write(
        &qexed_path,
        r#"
version = 0
update_check = true
language = "zh-CN"

[server.world]
view_distance = 3
"#,
    )?;
    std::fs::write(
        split_dir.join("world.toml"),
        r#"
[server.world]
path = "split-world"
read_only = true
generator = "vanilla_noise"
generator_preset = "minecraft:overworld"
seed = 42
game_mode = "creative"
spawn_protection_radius = 0
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 12
chunk_load_parallelism = 8
simulation_distance = 6
light = "dynamic"
light_algorithm = "fast"

[server.world.spawn]
x = 1.0
y = 80.0
z = 2.0
yaw = 0.0
pitch = 0.0
"#,
    )?;

    let config =
        Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(false), Some(dir.clone()))?;

    assert_eq!(config.server.world.path, "split-world");
    assert!(config.server.world.read_only);
    assert_eq!(config.server.world.view_distance, 12);
    assert_eq!(config.server.world.seed, 42);

    let qexed = std::fs::read_to_string(&qexed_path)?;
    let world = std::fs::read_to_string(split_dir.join("world.toml"))?;
    assert!(!qexed.contains("[server.world]"));
    assert!(world.contains("path = \"split-world\""));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
