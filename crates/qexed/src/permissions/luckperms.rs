use std::collections::HashSet;

use anyhow::{Context, Result};
use async_trait::async_trait;
use qexed_config::app::qexed::server::Permissions as PermissionConfig;

use super::{
    PermissionSnapshot, PermissionStore,
    rules::{
        PermissionNode, contexts_match, normalize_context, normalize_group, validate_sql_identifier,
    },
};
#[derive(Debug)]
pub(super) struct LuckPermsMysqlStore {
    pool: mysql_async::Pool,
    tables: LuckPermsTables,
    server: String,
    world: String,
    default_group: String,
}

impl LuckPermsMysqlStore {
    pub(super) async fn new(config: &PermissionConfig) -> Result<Self> {
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

    async fn grant_user_permission(
        &self,
        uuid: uuid::Uuid,
        username: &str,
        permission: &str,
        value: bool,
    ) -> Result<()> {
        use mysql_async::{params, prelude::Queryable};

        let mut conn = self.pool.get_conn().await?;
        conn.exec_drop(
            format!(
                "INSERT INTO `{}` (`uuid`, `username`, `primary_group`)
                 VALUES (:uuid, :username, :primary_group)
                 ON DUPLICATE KEY UPDATE `username` = VALUES(`username`)",
                self.tables.players
            ),
            params! {
                "uuid" => uuid.to_string(),
                "username" => username,
                "primary_group" => &self.default_group,
            },
        )
        .await?;
        conn.exec_drop(
            format!(
                "DELETE FROM `{}` WHERE `uuid` = :uuid AND `permission` = :permission",
                self.tables.user_permissions
            ),
            params! {
                "uuid" => uuid.to_string(),
                "permission" => permission,
            },
        )
        .await?;
        conn.exec_drop(
            format!(
                "INSERT INTO `{}` \
                 (`uuid`, `permission`, `value`, `server`, `world`, `expiry`, `contexts`)
                 VALUES (:uuid, :permission, :value, 'global', 'global', 0, '{{}}')",
                self.tables.user_permissions
            ),
            params! {
                "uuid" => uuid.to_string(),
                "permission" => permission,
                "value" => value,
            },
        )
        .await?;
        Ok(())
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
pub(super) struct LuckPermsTables {
    user_permissions: String,
    group_permissions: String,
    players: String,
}

impl LuckPermsTables {
    pub(super) fn new(prefix: &str) -> Result<Self> {
        validate_sql_identifier(prefix, "LuckPerms table prefix")?;
        Ok(Self {
            user_permissions: format!("{prefix}user_permissions"),
            group_permissions: format!("{prefix}group_permissions"),
            players: format!("{prefix}players"),
        })
    }
}
