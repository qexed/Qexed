use crate::config::GameplayConfig;
use qexed_protocol::types::Slot;

use super::items;

pub(in crate::gameplay) fn damage_item(
    slot: &mut Slot,
    player: &qexed_player::OnlinePlayer,
    plugins: &qexed_plugins::PluginManager,
    config: &GameplayConfig,
    reason: &str,
    amount: i32,
) -> bool {
    if !config.durability || items::is_unbreakable(slot) || slot.item_count.0 <= 0 {
        return false;
    }
    let (damage, max_damage) = items::durability(slot);
    if max_damage <= 0 {
        return false;
    }

    let mut amount = amount.max(0);
    if amount == 0 {
        return false;
    }
    if config.enchantments {
        amount = apply_unbreaking(slot, amount);
    }

    let response = plugins.apply_item_durability(qexed_plugins::api::ItemDurabilityQuery {
        player: crate::plugin_bridge::player_payload_owned(player),
        item: items::item_stack_payload(slot),
        reason: reason.to_string(),
        amount,
    });
    if response.cancel {
        return false;
    }
    if let Some(plugin_amount) = response.amount {
        amount = plugin_amount.max(0);
    }
    if amount == 0 {
        return false;
    }

    let next_damage = damage.saturating_add(amount);
    if next_damage >= max_damage {
        *slot = crate::inventory::empty_slot();
    } else {
        items::apply_damage_component(slot, next_damage, max_damage);
    }
    true
}

fn apply_unbreaking(slot: &Slot, amount: i32) -> i32 {
    let level = items::enchantment_level(slot, "minecraft:unbreaking").max(0);
    if level <= 0 {
        return amount;
    }
    let mut damaged = 0;
    for _ in 0..amount {
        let chance = 1.0 / f64::from(level + 1);
        if rand::Rng::gen_bool(&mut rand::thread_rng(), chance) {
            damaged += 1;
        }
    }
    damaged
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::net_types::{GameProfile, VarInt};
    use qexed_protocol::types::{ComponentsToAdd, minecraft};

    /// 空插件管理器（v6 的 empty_for_tests 是 qexed_plugins crate 内 cfg(test)，
    /// 跨 crate 不可见；从无插件目录构建等价实例）。
    fn empty_plugins() -> qexed_plugins::PluginManager {
        let dir = std::env::temp_dir().join(format!(
            "qexed-play-durability-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let manager = qexed_plugins::PluginManager::from_dir(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        manager
    }

    #[test]
    fn durability_component_increments_damage() {
        let mut slot = crate::inventory::simple_item(
            crate::inventory::item_id_for_name("minecraft:iron_pickaxe").unwrap_or(934),
            1,
        );
        items::apply_damage_component(&mut slot, 0, 250);
        let player = qexed_player::OnlinePlayer {
            profile: GameProfile {
                uuid: uuid::Uuid::nil(),
                username: "test".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            game_mode: 0,
            position: Default::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
            displayed_skin_parts: qexed_player::DEFAULT_DISPLAYED_SKIN_PARTS,
        };
        let plugins = empty_plugins();
        assert!(damage_item(
            &mut slot,
            &player,
            &plugins,
            &crate::config::GameplayConfig::default(),
            "mine",
            1
        ));
        assert_eq!(items::durability(&slot).0, 1);
    }

    #[test]
    fn unbreakable_item_does_not_damage() {
        let mut slot = crate::inventory::simple_item(1, 1);
        slot.components_to_add = Some(vec![
            ComponentsToAdd::MinecraftMaxDamage(minecraft::MaxDamage {
                max_damage: VarInt(10),
            }),
            ComponentsToAdd::MinecraftUnbreakable(minecraft::Unbreakable),
        ]);
        slot.number_of_components_to_add = Some(VarInt(2));
        let player = qexed_player::OnlinePlayer {
            profile: GameProfile {
                uuid: uuid::Uuid::nil(),
                username: "test".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            game_mode: 0,
            position: Default::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
            displayed_skin_parts: qexed_player::DEFAULT_DISPLAYED_SKIN_PARTS,
        };
        let plugins = empty_plugins();
        assert!(!damage_item(
            &mut slot,
            &player,
            &plugins,
            &crate::config::GameplayConfig::default(),
            "test",
            1
        ));
    }
}
