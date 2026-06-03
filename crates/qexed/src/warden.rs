use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use qexed_config::{
    app::qexed_warden::{QexedWarden, data::BanRecord},
    tool::AppConfigTrait,
};

#[derive(Debug)]
pub struct WardenManager {
    config: Mutex<QexedWarden>,
}

impl WardenManager {
    pub fn from_config(config: QexedWarden) -> Self {
        Self {
            config: Mutex::new(config),
        }
    }

    pub fn ban_for(&self, uuid: uuid::Uuid) -> Option<BanRecord> {
        let config = self.config.lock().expect("warden config lock poisoned");
        if let Some(record) = config
            .data
            .simple
            .bans
            .iter()
            .find(|record| record.uuid == uuid)
        {
            return Some(record.clone());
        }

        config
            .data
            .simple
            .player_list
            .contains(&uuid)
            .then(|| BanRecord {
                uuid,
                username: String::new(),
                reason: "Banned".to_string(),
                permanent: true,
                created_at_unix_secs: 0,
            })
    }

    pub fn permanently_ban(
        &self,
        profile: &qexed_packet::net_types::GameProfile,
        reason: impl Into<String>,
    ) -> anyhow::Result<BanRecord> {
        let reason = reason.into();
        let mut config = self.config.lock().expect("warden config lock poisoned");
        if !config.data.simple.player_list.contains(&profile.uuid) {
            config.data.simple.player_list.push(profile.uuid);
        }

        let record = BanRecord {
            uuid: profile.uuid,
            username: profile.username.clone(),
            reason,
            permanent: true,
            created_at_unix_secs: current_unix_secs(),
        };

        if let Some(existing) = config
            .data
            .simple
            .bans
            .iter_mut()
            .find(|existing| existing.uuid == profile.uuid)
        {
            *existing = record.clone();
        } else {
            config.data.simple.bans.push(record.clone());
        }

        config.save_to_config(None, None, None)?;
        Ok(record)
    }
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
    use qexed_config::app::qexed_warden::QexedWarden;

    #[test]
    fn legacy_player_list_is_treated_as_permanent_ban() {
        let uuid = uuid::Uuid::new_v4();
        let mut config = QexedWarden::default();
        config.data.simple.player_list.push(uuid);
        let warden = WardenManager::from_config(config);

        let ban = warden.ban_for(uuid).expect("legacy ban should be visible");

        assert!(ban.permanent);
        assert_eq!(ban.uuid, uuid);
    }
}
