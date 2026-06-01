use qexed_config::app::qexed::server::Gameplay;
use qexed_protocol::types::Slot;

use super::items;

pub(in crate::play) fn damage_item(
    slot: &mut Slot,
    player: &crate::players::OnlinePlayer,
    plugins: &crate::plugins::PluginManager,
    config: &Gameplay,
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

    let response = plugins.apply_item_durability(crate::plugins::ItemDurabilityQuery {
        player: qexed_plugin_api::player_payload_owned(player),
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

    #[test]
    fn durability_component_increments_damage() {
        let mut slot = crate::inventory::simple_item(
            crate::inventory::item_id_for_name("minecraft:iron_pickaxe").unwrap_or(934),
            1,
        );
        items::apply_damage_component(&mut slot, 0, 250);
        let player = crate::players::OnlinePlayer {
            profile: GameProfile {
                uuid: uuid::Uuid::nil(),
                username: "test".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            position: Default::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
        };
        let plugins = crate::plugins::PluginManager::empty_for_tests();
        assert!(damage_item(
            &mut slot,
            &player,
            &plugins,
            &qexed_config::app::qexed::server::Gameplay::default(),
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
        let player = crate::players::OnlinePlayer {
            profile: GameProfile {
                uuid: uuid::Uuid::nil(),
                username: "test".to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            position: Default::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh-CN".to_string(),
        };
        let plugins = crate::plugins::PluginManager::empty_for_tests();
        assert!(!damage_item(
            &mut slot,
            &player,
            &plugins,
            &qexed_config::app::qexed::server::Gameplay::default(),
            "test",
            1
        ));
    }
}
