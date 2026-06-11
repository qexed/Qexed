use crate::players::PlayerEvent;

pub(super) fn event_is_self(event: &PlayerEvent, profile_id: uuid::Uuid) -> bool {
    match event {
        PlayerEvent::Joined(player) => player.profile.uuid == profile_id,
        PlayerEvent::Left {
            profile_id: left_id,
            entity_id: _,
            username: _,
            dimension: _,
        } => *left_id == profile_id,
        PlayerEvent::Moved {
            profile_id: moved_id,
            entity_id: _,
            dimension: _,
            position: _,
        } => *moved_id == profile_id,
        PlayerEvent::Teleport {
            profile_id: target_id,
            dimension: _,
            position: _,
        } => *target_id == profile_id,
        PlayerEvent::Damage {
            profile_id: target_id,
            amount: _,
            kind: _,
            source_entity_id: _,
            source_position: _,
            knockback: _,
        } => *target_id == profile_id,
        PlayerEvent::PotionEffect {
            profile_id: target_id,
            effect: _,
            amplifier: _,
            duration_ticks: _,
            source_entity_id: _,
            source_position: _,
            knockback: _,
        } => *target_id == profile_id,
        PlayerEvent::GameModeChanged {
            profile_id: target_id,
            username: _,
            game_mode: _,
        } => *target_id == profile_id,
        PlayerEvent::GiveItem {
            profile_id: target_id,
            item: _,
            item_name: _,
        } => *target_id == profile_id,
        PlayerEvent::DimensionChanged {
            profile_id: target_id,
            entity_id: _,
            old_dimension: _,
            player: _,
        } => *target_id == profile_id,
        PlayerEvent::EquipmentChanged {
            profile_id: changed_id,
            entity_id: _,
            dimension: _,
            slots: _,
        } => *changed_id == profile_id,
        PlayerEvent::BlockChanged {
            profile_id: changed_id,
            dimension: _,
            position: _,
            block_state: _,
            light_update: _,
        } => *changed_id == profile_id,
        PlayerEvent::ClientboundPackets { packets: _ } => false,
    }
}

pub(super) fn player_event_message(
    config: &qexed_config::app::qexed::Qexed,
    event: &PlayerEvent,
) -> Option<String> {
    let messages = &config.server.player_messages;
    if !messages.enable {
        return None;
    }

    match event {
        PlayerEvent::Joined(player) => Some(render_player_message(
            &messages.join,
            &player.profile.username,
        )),
        PlayerEvent::Left { username, .. } => {
            Some(render_player_message(&messages.leave, username))
        }
        PlayerEvent::GameModeChanged { .. } | PlayerEvent::GiveItem { .. } => None,
        PlayerEvent::ClientboundPackets { packets: _ } => None,
        _ => None,
    }
}

fn render_player_message(template: &str, username: &str) -> String {
    template.replace("{player}", username)
}
