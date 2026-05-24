use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use qexed_config::app::qexed::server::{PermissionEngine, Permissions as PermissionConfig};

const GROUP_PREFIX: &str = "group.";
const WILDCARD: &str = "*";

#[derive(Debug, Clone)]
pub struct PermissionManager {
    allow_by_default: bool,
    denied_message: String,
    store: std::sync::Arc<dyn PermissionStore>,
}

impl PermissionManager {
    pub async fn from_config(config: &PermissionConfig) -> Result<Self> {
        let store: std::sync::Arc<dyn PermissionStore> = match config.engine {
            PermissionEngine::Local => std::sync::Arc::new(LocalPermissionStore::new(config)?),
            PermissionEngine::LuckpermsMysql => {
                std::sync::Arc::new(LuckPermsMysqlStore::new(config).await?)
            }
        };

        Ok(Self {
            allow_by_default: config.allow_by_default,
            denied_message: config.denied_message.clone(),
            store,
        })
    }

    pub fn denied_message(&self) -> &str {
        &self.denied_message
    }

    pub async fn can_run_command(
        &self,
        profile: &qexed_packet::net_types::GameProfile,
        command: &str,
    ) -> Result<bool> {
        let Some(permission) = crate::commands::permission_node(command) else {
            return Ok(self.allow_by_default);
        };

        self.check(profile.uuid, &permission).await
    }

    pub async fn check(&self, uuid: uuid::Uuid, permission: &str) -> Result<bool> {
        let snapshot = self.store.load_user(uuid).await?;
        let result = resolve_permission(&snapshot.nodes, permission);
        Ok(result.unwrap_or(self.allow_by_default))
    }
}

#[async_trait]
trait PermissionStore: Send + Sync + std::fmt::Debug {
    async fn load_user(&self, uuid: uuid::Uuid) -> Result<PermissionSnapshot>;
}

#[derive(Debug)]
struct LocalPermissionStore {
    path: PathBuf,
    default_group: String,
}

impl LocalPermissionStore {
    fn new(config: &PermissionConfig) -> Result<Self> {
        Ok(Self {
            path: local_permission_path(&config.local_path)?,
            default_group: normalize_group(&config.default_group),
        })
    }

    async fn load_config(&self) -> Result<LocalPermissionsFile> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => toml::from_str::<LocalPermissionsFile>(&content)
                .map(LocalPermissionsFile::normalized)
                .with_context(|| format!("parse local permission file {}", self.path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(LocalPermissionsFile::default())
            }
            Err(err) => Err(err)
                .with_context(|| format!("read local permission file {}", self.path.display())),
        }
    }
}

#[async_trait]
impl PermissionStore for LocalPermissionStore {
    async fn load_user(&self, uuid: uuid::Uuid) -> Result<PermissionSnapshot> {
        let config = self.load_config().await?;
        let user_key = uuid.to_string();
        let user = config.users.get(&user_key);
        let mut nodes = Vec::new();
        let mut pending_groups = Vec::new();
        pending_groups.push(self.default_group.clone());
        if let Some(user) = user {
            pending_groups.extend(user.groups.iter().map(|group| normalize_group(group)));
            pending_groups.extend(
                user.permissions
                    .iter()
                    .filter_map(PermissionNode::inherited_group)
                    .map(str::to_string),
            );
        }

        let mut seen = HashSet::new();
        let mut index = 0;
        while let Some(group) = pending_groups.get(index).cloned() {
            index += 1;
            if !seen.insert(group.clone()) {
                continue;
            }

            let Some(group_data) = config.groups.get(&group) else {
                continue;
            };
            pending_groups.extend(group_data.groups.iter().map(|group| normalize_group(group)));
            pending_groups.extend(
                group_data
                    .permissions
                    .iter()
                    .filter_map(PermissionNode::inherited_group)
                    .map(str::to_string),
            );
            nodes.extend(group_data.permissions.clone());
        }

        if let Some(user) = user {
            nodes.extend(user.permissions.clone());
        }

        Ok(PermissionSnapshot { nodes })
    }
}

#[derive(Debug, Default)]
struct PermissionSnapshot {
    nodes: Vec<PermissionNode>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct LocalPermissionsFile {
    #[serde(default)]
    groups: BTreeMap<String, LocalPermissionHolder>,
    #[serde(default)]
    users: BTreeMap<String, LocalPermissionHolder>,
}

impl LocalPermissionsFile {
    fn normalized(self) -> Self {
        Self {
            groups: self
                .groups
                .into_iter()
                .map(|(group, holder)| (normalize_group(group), holder))
                .collect(),
            users: self
                .users
                .into_iter()
                .map(|(user, holder)| (normalize_user_key(user), holder))
                .collect(),
        }
    }
}

#[derive(Debug, Default, serde::Deserialize)]
struct LocalPermissionHolder {
    #[serde(default)]
    permissions: Vec<PermissionNode>,
    #[serde(default)]
    groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PermissionNode {
    permission: String,
    value: bool,
}

impl<'de> serde::Deserialize<'de> for PermissionNode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum NodeValue {
            Text(String),
            Detailed { permission: String, value: bool },
        }

        match NodeValue::deserialize(deserializer)? {
            NodeValue::Text(permission) => Ok(PermissionNode::new(permission, true)),
            NodeValue::Detailed { permission, value } => Ok(PermissionNode::new(permission, value)),
        }
    }
}

impl PermissionNode {
    fn new(permission: impl Into<String>, value: bool) -> Self {
        Self {
            permission: normalize_node(permission.into()),
            value,
        }
    }

    fn inherited_group(&self) -> Option<&str> {
        self.value
            .then(|| self.permission.strip_prefix(GROUP_PREFIX))
            .flatten()
            .filter(|group| !group.is_empty())
    }
}

#[derive(Debug)]
struct LuckPermsMysqlStore {
    pool: mysql_async::Pool,
    tables: LuckPermsTables,
    server: String,
    world: String,
    default_group: String,
}

impl LuckPermsMysqlStore {
    async fn new(config: &PermissionConfig) -> Result<Self> {
        config.mysql.validate().map_err(anyhow::Error::msg)?;
        let tables = LuckPermsTables::new(&config.table_prefix)?;
        let opts = mysql_async::Opts::from_url(&config.mysql.connection_string())?;
        Ok(Self {
            pool: mysql_async::Pool::new(opts),
            tables,
            server: normalize_context(&config.server),
            world: normalize_context(&config.world),
            default_group: normalize_group(&config.default_group),
        })
    }

    async fn load_user_nodes(&self, uuid: uuid::Uuid) -> Result<Vec<PermissionNode>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<LuckPermsNodeRow> = conn
            .exec(
                format!(
                    "SELECT `permission`, `value`, `contexts` FROM `{}` \
                     WHERE `uuid` = :uuid \
                       AND (`expiry` = 0 OR `expiry` > UNIX_TIMESTAMP()) \
                       AND (`server` = 'global' OR `server` = :server) \
                       AND (`world` = 'global' OR `world` = :world)",
                    self.tables.user_permissions
                ),
                params! {
                    "uuid" => uuid.to_string(),
                    "server" => &self.server,
                    "world" => &self.world,
                },
            )
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(permission, value, contexts)| {
                contexts_match(&contexts).then(|| PermissionNode::new(permission, value))
            })
            .collect())
    }

    async fn load_primary_group(&self, uuid: uuid::Uuid) -> Result<Option<String>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let group: Option<String> = conn
            .exec_first(
                format!(
                    "SELECT `primary_group` FROM `{}` WHERE `uuid` = :uuid LIMIT 1",
                    self.tables.players
                ),
                params! { "uuid" => uuid.to_string() },
            )
            .await?;

        Ok(group.map(normalize_group).filter(|group| !group.is_empty()))
    }

    async fn load_group_nodes(&self, group: &str) -> Result<Vec<PermissionNode>> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<LuckPermsNodeRow> = conn
            .exec(
                format!(
                    "SELECT `permission`, `value`, `contexts` FROM `{}` \
                     WHERE `name` = :name \
                       AND (`expiry` = 0 OR `expiry` > UNIX_TIMESTAMP()) \
                       AND (`server` = 'global' OR `server` = :server) \
                       AND (`world` = 'global' OR `world` = :world)",
                    self.tables.group_permissions
                ),
                params! {
                    "name" => group,
                    "server" => &self.server,
                    "world" => &self.world,
                },
            )
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(permission, value, contexts)| {
                contexts_match(&contexts).then(|| PermissionNode::new(permission, value))
            })
            .collect())
    }
}

#[async_trait]
impl PermissionStore for LuckPermsMysqlStore {
    async fn load_user(&self, uuid: uuid::Uuid) -> Result<PermissionSnapshot> {
        let user_nodes = self.load_user_nodes(uuid).await?;
        let mut nodes = Vec::new();
        let mut pending_groups = Vec::new();
        pending_groups.push(self.default_group.clone());
        if let Some(primary_group) = self.load_primary_group(uuid).await? {
            pending_groups.push(primary_group);
        }
        pending_groups.extend(
            user_nodes
                .iter()
                .filter_map(PermissionNode::inherited_group)
                .map(str::to_string),
        );

        let mut seen = HashSet::new();
        let mut index = 0;
        while let Some(group) = pending_groups.get(index).cloned() {
            index += 1;
            if !seen.insert(group.clone()) {
                continue;
            }

            let group_nodes = self
                .load_group_nodes(&group)
                .await
                .with_context(|| format!("load LuckPerms group permissions: {group}"))?;
            pending_groups.extend(
                group_nodes
                    .iter()
                    .filter_map(PermissionNode::inherited_group)
                    .map(str::to_string),
            );
            nodes.extend(group_nodes);
        }
        nodes.extend(user_nodes);

        Ok(PermissionSnapshot { nodes })
    }
}

type LuckPermsNodeRow = (String, bool, String);

impl Drop for LuckPermsMysqlStore {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let _ = pool.disconnect().await;
        });
    }
}

#[derive(Debug)]
struct LuckPermsTables {
    user_permissions: String,
    group_permissions: String,
    players: String,
}

impl LuckPermsTables {
    fn new(prefix: &str) -> Result<Self> {
        validate_sql_identifier(prefix, "LuckPerms table prefix")?;
        Ok(Self {
            user_permissions: format!("{prefix}user_permissions"),
            group_permissions: format!("{prefix}group_permissions"),
            players: format!("{prefix}players"),
        })
    }
}

fn resolve_permission(nodes: &[PermissionNode], permission: &str) -> Option<bool> {
    let permission = normalize_node(permission);

    for node in nodes.iter().rev() {
        if node_matches(&node.permission, &permission) {
            return Some(node.value);
        }
    }

    None
}

fn node_matches(node: &str, permission: &str) -> bool {
    node == permission || node == WILDCARD || wildcard_matches(node, permission)
}

fn wildcard_matches(node: &str, permission: &str) -> bool {
    node.strip_suffix(".*")
        .is_some_and(|prefix| permission == prefix || permission.starts_with(&format!("{prefix}.")))
}

fn normalize_node(permission: impl AsRef<str>) -> String {
    permission.as_ref().trim().to_ascii_lowercase()
}

fn normalize_group(group: impl AsRef<str>) -> String {
    group.as_ref().trim().to_ascii_lowercase()
}

fn normalize_user_key(user: impl AsRef<str>) -> String {
    user.as_ref().trim().to_ascii_lowercase()
}

fn normalize_context(context: impl AsRef<str>) -> String {
    let context = context.as_ref().trim();
    if context.is_empty() {
        "global".to_string()
    } else {
        context.to_string()
    }
}

fn local_permission_path(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        anyhow::bail!("local permission file path cannot be empty");
    }
    Ok(PathBuf::from(path))
}

fn contexts_match(contexts: &str) -> bool {
    let contexts = contexts.trim();
    if contexts.is_empty() || contexts == "{}" {
        return true;
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(contexts) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };

    object.is_empty()
}

fn validate_sql_identifier(value: &str, label: &str) -> Result<()> {
    let valid = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        anyhow::bail!("{label} may only contain ASCII letters, digits, and underscore")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn local_engine_missing_file_uses_allow_by_default_without_mysql() {
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
}
