//! 玩家事件过滤/广播文案（v4 play/events.rs 迁移）。
//!
//! v6 适配：PlayerEvent 来自 qexed_player（v4 在 crate::players）；
//! v6 枚举缺少 EquipmentChanged（改名为 SetEquipmentChanged），配置用 crate::config。

use qexed_player::PlayerEvent;

use crate::config::PlayerMessages;

pub fn event_is_self(event: &PlayerEvent, profile_id: uuid::Uuid) -> bool {
    match event {
        PlayerEvent::Joined(player) => player.profile.uuid == profile_id,
        PlayerEvent::Left {
            profile_id: left_id,
            ..
        } => *left_id == profile_id,
        PlayerEvent::Moved {
            profile_id: moved_id,
            ..
        } => *moved_id == profile_id,
        PlayerEvent::Teleport {
            profile_id: target_id,
            ..
        } => *target_id == profile_id,
        PlayerEvent::Damage {
            profile_id: target_id,
            ..
        } => *target_id == profile_id,
        PlayerEvent::PotionEffect {
            profile_id: target_id,
            ..
        } => *target_id == profile_id,
        PlayerEvent::GameModeChanged {
            profile_id: target_id,
            ..
        } => *target_id == profile_id,
        PlayerEvent::GiveItem {
            profile_id: target_id,
            ..
        } => *target_id == profile_id,
        PlayerEvent::ProjectileHitPlayer(event) => event.shooter_profile_id == profile_id,
        PlayerEvent::Animation {
            profile_id: animated_id,
            ..
        } => *animated_id == profile_id,
        PlayerEvent::DimensionChanged {
            profile_id: target_id,
            ..
        } => *target_id == profile_id,
        PlayerEvent::SetEquipmentChanged {
            profile_id: changed_id,
            ..
        } => *changed_id == profile_id,
        PlayerEvent::BlockChanged {
            profile_id: changed_id,
            ..
        } => *changed_id == profile_id,
        PlayerEvent::BlockChanges {
            profile_id: changed_id,
            ..
        } => *changed_id == profile_id,
        PlayerEvent::ClientboundPackets { .. } => false,
    }
}

/// 进出服广播文案（v4 player_event_message；仅 Joined/Left 产生消息）。
pub fn player_event_message(
    config: &PlayerMessages,
    event: &PlayerEvent,
) -> Option<String> {
    if !config.enable {
        return None;
    }

    match event {
        PlayerEvent::Joined(player) => {
            Some(render_player_message(&config.join, &player.profile.username))
        }
        PlayerEvent::Left { username, .. } => Some(render_player_message(&config.leave, username)),
        _ => None,
    }
}

fn render_player_message(template: &str, username: &str) -> String {
    template.replace("{player}", username)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::net_types::GameProfile;
    use qexed_player::OnlinePlayer;
    use qexed_protocol::types::EntityPosition;

    fn player(uuid: uuid::Uuid) -> OnlinePlayer {
        OnlinePlayer {
            profile: GameProfile {
                uuid,
                username: "Player".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            game_mode: 0,
            position: EntityPosition::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
            displayed_skin_parts: 0x7f,
        }
    }

    #[test]
    fn join_message_renders_player_name() {
        let id = uuid::Uuid::from_u128(7);
        let message = player_event_message(
            &PlayerMessages::default(),
            &PlayerEvent::Joined(player(id)),
        )
        .unwrap();

        assert_eq!(message, "Player joined the game");
    }

    #[test]
    fn event_is_self_matches_profile_id() {
        let id = uuid::Uuid::from_u128(7);
        let other = uuid::Uuid::from_u128(8);
        assert!(event_is_self(&PlayerEvent::Joined(player(id)), id));
        assert!(!event_is_self(&PlayerEvent::Joined(player(id)), other));
    }
}
