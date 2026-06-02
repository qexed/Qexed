use super::*;
use super::{
    luckperms::LuckPermsTables,
    rules::{PermissionNode, contexts_match, local_permission_path, resolve_permission},
};

#[test]
fn command_names_map_to_qexed_permission_nodes() {
    assert_eq!(
        crate::commands::permission_node("list").as_deref(),
        Some("qexed.command.list")
    );
    assert_eq!(
        crate::commands::permission_node("/help ignored").as_deref(),
        Some("qexed.command.help")
    );
    assert_eq!(
        crate::commands::permission_node("/tp Player").as_deref(),
        Some("qexed.command.teleport")
    );
    assert_eq!(
        crate::commands::permission_node("teleport Player").as_deref(),
        Some("qexed.command.teleport")
    );
    assert_eq!(crate::commands::permission_node("   "), None);
}

#[test]
fn direct_permission_nodes_are_resolved_last_write_wins() {
    let nodes = vec![
        PermissionNode::new("qexed.command.list", true),
        PermissionNode::new("qexed.command.list", false),
    ];

    assert_eq!(
        resolve_permission(&nodes, "qexed.command.list"),
        Some(false)
    );
}

#[test]
fn user_nodes_override_inherited_group_nodes_when_ordered_last() {
    let nodes = vec![
        PermissionNode::new("qexed.command.list", false),
        PermissionNode::new("qexed.command.list", true),
    ];

    assert_eq!(resolve_permission(&nodes, "qexed.command.list"), Some(true));
}

#[test]
fn wildcard_permission_nodes_cover_children() {
    let nodes = vec![PermissionNode::new("qexed.command.*", true)];

    assert_eq!(resolve_permission(&nodes, "qexed.command.list"), Some(true));
    assert_eq!(resolve_permission(&nodes, "qexed.other.list"), None);
}

#[test]
fn global_wildcard_permission_node_matches_anything() {
    let nodes = vec![PermissionNode::new("*", true)];

    assert_eq!(resolve_permission(&nodes, "qexed.command.help"), Some(true));
}

#[test]
fn inherited_group_nodes_are_detected() {
    let node = PermissionNode::new("group.admin", true);

    assert_eq!(node.inherited_group(), Some("admin"));
    assert_eq!(
        PermissionNode::new("group.admin", false).inherited_group(),
        None
    );
}

#[test]
fn local_permission_paths_are_runtime_relative() {
    assert_eq!(
        local_permission_path(" config/qexed_permissions.toml ")
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/"),
        "config/qexed_permissions.toml"
    );
    assert!(local_permission_path(" ").is_err());
}

#[tokio::test]
async fn local_engine_loads_groups_inheritance_and_user_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permissions.toml");
    let uuid = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
    std::fs::write(
        &path,
        format!(
            r#"
[groups.DEFAULT]
permissions = [
    "group.Builder",
    {{ permission = "qexed.command.help", value = false }},
]
groups = []

[groups.Builder]
permissions = ["qexed.command.list"]
groups = ["Admin"]

[groups.ADMIN]
permissions = ["qexed.command.*"]
groups = []

[users."{}"]
permissions = [
    {{ permission = "qexed.command.list", value = false }},
    "qexed.command.help",
]
groups = []
"#,
            uuid.to_string().to_ascii_uppercase()
        ),
    )
    .unwrap();

    let mut config = PermissionConfig::default();
    config.engine = PermissionEngine::Local;
    config.local_path = path.to_string_lossy().to_string();
    config.default_group = "Default".to_string();
    config.allow_by_default = false;

    let manager = PermissionManager::from_config(&config).await.unwrap();

    assert!(!manager.check(uuid, "qexed.command.list").await.unwrap());
    assert!(manager.check(uuid, "qexed.command.help").await.unwrap());
    assert!(manager.check(uuid, "qexed.command.stop").await.unwrap());
    assert!(!manager.check(uuid, "qexed.other.stop").await.unwrap());
}

#[tokio::test]
async fn local_engine_default_policy_allows_help_and_denies_teleport() {
    let dir = tempfile::tempdir().unwrap();
    let uuid = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let mut config = PermissionConfig::default();
    config.engine = PermissionEngine::Local;
    config.local_path = dir
        .path()
        .join("missing-permissions.toml")
        .to_string_lossy()
        .to_string();

    let manager = PermissionManager::from_config(&config).await.unwrap();
    let profile = qexed_packet::net_types::GameProfile {
        uuid,
        username: "player".to_string(),
        properties: Vec::new(),
    };

    assert!(manager.can_run_command(&profile, "help").await.unwrap());
    assert!(manager.can_run_command(&profile, "/list").await.unwrap());
    assert!(
        !manager
            .can_run_command(&profile, "tp player")
            .await
            .unwrap()
    );
    assert!(
        !manager
            .can_run_command(&profile, "teleport player")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn console_has_wildcard_permission() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = PermissionConfig::default();
    config.engine = PermissionEngine::Local;
    config.local_path = dir
        .path()
        .join("missing-permissions.toml")
        .to_string_lossy()
        .to_string();
    config.allow_by_default = false;

    let manager = PermissionManager::from_config(&config).await.unwrap();

    assert!(manager.can_run_console_command("tp Steve Alex"));
    assert!(manager.can_run_console_command("/any_plugin_command"));
}

#[tokio::test]
async fn local_engine_empty_policy_file_uses_builtin_default_group() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("qexed_permissions.toml");
    std::fs::write(
        &path,
        r#"
[permissions]
engine = "local"
"#,
    )
    .unwrap();

    let uuid = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let mut config = PermissionConfig::default();
    config.engine = PermissionEngine::Local;
    config.local_path = path.to_string_lossy().to_string();

    let manager = PermissionManager::from_config(&config).await.unwrap();

    assert!(manager.check(uuid, "qexed.command.help").await.unwrap());
    assert!(!manager.check(uuid, "qexed.command.teleport").await.unwrap());
}

#[tokio::test]
async fn local_engine_missing_file_can_still_allow_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let uuid = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let mut config = PermissionConfig::default();
    config.engine = PermissionEngine::Local;
    config.local_path = dir
        .path()
        .join("missing-permissions.toml")
        .to_string_lossy()
        .to_string();
    config.allow_by_default = true;
    config.mysql.ip = String::new();
    config.mysql.username = String::new();
    config.mysql.database = String::new();

    let manager = PermissionManager::from_config(&config).await.unwrap();

    assert!(manager.check(uuid, "qexed.command.list").await.unwrap());
    assert!(manager.check(uuid, "qexed.command.teleport").await.unwrap());
}

#[test]
fn luckperms_table_prefix_rejects_unsafe_identifiers() {
    assert!(LuckPermsTables::new("").is_ok());
    assert!(LuckPermsTables::new("luckperms_").is_ok());
    assert!(LuckPermsTables::new("luckperms;DROP").is_err());
    assert!(LuckPermsTables::new("luckperms-").is_err());
}

#[test]
fn luckperms_extra_contexts_must_be_empty() {
    assert!(contexts_match("{}"));
    assert!(contexts_match(""));
    assert!(!contexts_match(r#"{"region":"spawn"}"#));
    assert!(!contexts_match("not json"));
}
