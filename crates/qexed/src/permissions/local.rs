use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use qexed_config::app::qexed::server::Permissions as PermissionConfig;

use super::{
    PermissionSnapshot, PermissionStore,
    rules::{PermissionNode, local_permission_path, normalize_group, normalize_user_key},
};

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

fn builtin_default_group_permissions() -> LocalPermissionHolder {
    LocalPermissionHolder {
        permissions: vec![
            PermissionNode::new("qexed.command.help", true),
            PermissionNode::new("qexed.command.list", true),
        ],
        groups: Vec::new(),
    }
}

#[derive(Debug)]
pub(super) struct LocalPermissionStore {
    path: PathBuf,
    default_group: String,
}

impl LocalPermissionStore {
    pub(super) fn new(config: &PermissionConfig) -> Result<Self> {
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
        let mut config = self.load_config().await?;
        if config.groups.is_empty() {
            config.groups.insert(
                self.default_group.clone(),
                builtin_default_group_permissions(),
            );
        }

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
