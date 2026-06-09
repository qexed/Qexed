mod local;
mod luckperms;
mod rules;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use qexed_config::app::qexed::server::{PermissionEngine, Permissions as PermissionConfig};

use local::LocalPermissionStore;
use luckperms::LuckPermsMysqlStore;
use rules::{PermissionSnapshot, resolve_permission};

#[derive(Debug, Clone)]
pub struct PermissionManager {
    allow_by_default: bool,
    denied_message: String,
    store: Arc<dyn PermissionStore>,
}

impl PermissionManager {
    pub async fn from_config(config: &PermissionConfig) -> Result<Self> {
        let store: Arc<dyn PermissionStore> = match config.engine {
            PermissionEngine::Local => Arc::new(LocalPermissionStore::new(config)?),
            PermissionEngine::LuckpermsMysql => Arc::new(LuckPermsMysqlStore::new(config).await?),
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

    pub fn can_run_console_command(&self, command: &str) -> bool {
        crate::commands::permission_node(command)
            .as_deref()
            .is_none_or(|permission| self.check_console(permission))
    }

    fn check_console(&self, _permission: &str) -> bool {
        true
    }

    pub async fn check(&self, uuid: uuid::Uuid, permission: &str) -> Result<bool> {
        let snapshot = self.store.load_user(uuid).await?;
        let result = resolve_permission(&snapshot.nodes, permission);
        Ok(result.unwrap_or(self.allow_by_default))
    }

    pub async fn grant_global_wildcard(&self, uuid: uuid::Uuid, username: &str) -> Result<()> {
        self.store
            .grant_user_permission(uuid, username, "*", true)
            .await
    }
}

#[async_trait]
trait PermissionStore: Send + Sync + std::fmt::Debug {
    async fn load_user(&self, uuid: uuid::Uuid) -> Result<PermissionSnapshot>;

    async fn grant_user_permission(
        &self,
        uuid: uuid::Uuid,
        username: &str,
        permission: &str,
        value: bool,
    ) -> Result<()>;
}
