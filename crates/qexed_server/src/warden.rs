//! Warden：封禁管理。
//! 迁移自 v4 crates/qexed/src/warden.rs；配置换成 v6 的 crate::config::WardenConfig，
//! 保存走 qexed_config::Config::create_file（v4 的 save_to_config 在 v6 不存在）。

use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use qexed_config::Config;
use qexed_packet::net_types::GameProfile;

use crate::config::{BanRecord, WardenConfig};
use crate::error::Result;

#[derive(Debug)]
pub struct WardenManager {
    config: Mutex<WardenConfig>,
}

impl WardenManager {
    pub fn from_config(config: WardenConfig) -> Self {
        Self {
            config: Mutex::new(config),
        }
    }

    /// 查询玩家的封禁记录：先查显式 bans，再把旧版 player_list 命中视为永久封禁。
    pub fn ban_for(&self, uuid: uuid::Uuid) -> Option<BanRecord> {
        let config = self.config.lock().expect("warden config lock poisoned");
        if let Some(record) = config.bans.iter().find(|record| record.uuid == uuid) {
            return Some(record.clone());
        }

        config.player_list.contains(&uuid).then(|| BanRecord {
            uuid,
            username: String::new(),
            reason: "Banned".to_string(),
            permanent: true,
            created_at_unix_secs: 0,
        })
    }

    /// 永久封禁一个玩家并持久化到 warden 配置文件。
    pub fn permanently_ban(
        &self,
        profile: &GameProfile,
        reason: impl Into<String>,
    ) -> Result<BanRecord> {
        let reason = reason.into();
        let mut config = self.config.lock().expect("warden config lock poisoned");
        if !config.player_list.contains(&profile.uuid) {
            config.player_list.push(profile.uuid);
        }

        let record = BanRecord {
            uuid: profile.uuid,
            username: profile.username.clone(),
            reason,
            permanent: true,
            created_at_unix_secs: current_unix_secs(),
        };

        if let Some(existing) = config.bans.iter_mut().find(|existing| existing.uuid == profile.uuid) {
            *existing = record.clone();
        } else {
            config.bans.push(record.clone());
        }

        persist(&config)?;
        Ok(record)
    }
}

/// 持久化 warden 配置（v6：Config::save_file 合并回写，保留未知字段）。
fn persist(config: &WardenConfig) -> Result<()> {
    Ok(WardenConfig::save_file(config)?)
}

fn current_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::WardenManager;
    use crate::config::WardenConfig;

    #[test]
    fn legacy_player_list_is_treated_as_permanent_ban() {
        let uuid = uuid::Uuid::new_v4();
        let mut config = WardenConfig::default();
        config.player_list.push(uuid);
        let warden = WardenManager::from_config(config);

        let ban = warden.ban_for(uuid).expect("legacy ban should be visible");

        assert!(ban.permanent);
        assert_eq!(ban.uuid, uuid);
    }
}
