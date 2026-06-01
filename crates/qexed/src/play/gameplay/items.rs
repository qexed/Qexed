use qexed_packet::net_types::VarInt;
use qexed_protocol::types::{ComponentsToAdd, Slot, minecraft};

use crate::plugins::{ItemEnchantment, ItemStackPayload, PluginEnchantment};

pub(super) fn item_stack_payload(slot: &Slot) -> ItemStackPayload {
    let item_id = slot.item_id.as_ref().map(|id| id.0).unwrap_or(-1);
    let (damage, max_damage) = durability(slot);
    ItemStackPayload {
        item_id,
        item_name: crate::inventory::item_name_for_id(item_id).unwrap_or_default(),
        count: slot.item_count.0,
        damage,
        max_damage,
        enchantments: enchantments(slot),
        plugin_enchantments: plugin_enchantments(slot),
    }
}

pub(super) fn enchantments(slot: &Slot) -> Vec<ItemEnchantment> {
    let mut result = Vec::new();
    let Some(components) = slot.components_to_add.as_ref() else {
        return result;
    };
    for component in components {
        let enchantments = match component {
            ComponentsToAdd::MinecraftEnchantments(enchantments) => &enchantments.enchantments,
            ComponentsToAdd::MinecraftStoredEnchantments(enchantments) => {
                &enchantments.enchantments
            }
            _ => continue,
        };
        for enchantment in enchantments {
            if enchantment.level.0 <= 0 {
                continue;
            }
            result.push(ItemEnchantment {
                id: enchantment_name(enchantment.enchantment.0).to_string(),
                level: enchantment.level.0,
            });
        }
    }
    result
}

pub(super) fn enchantment_level(slot: &Slot, name: &str) -> i32 {
    enchantments(slot)
        .into_iter()
        .find(|enchantment| enchantment.id == name)
        .map(|enchantment| enchantment.level)
        .unwrap_or(0)
}

pub(super) fn plugin_enchantments(slot: &Slot) -> Vec<PluginEnchantment> {
    let mut result = Vec::new();
    let Some(components) = slot.components_to_add.as_ref() else {
        return result;
    };
    for component in components {
        let ComponentsToAdd::MinecraftCustomData(custom_data) = component else {
            continue;
        };
        collect_plugin_enchantments(&custom_data.data, &mut result);
    }
    result
}

pub(super) fn durability(slot: &Slot) -> (i32, i32) {
    let mut damage = 0;
    let mut max_damage = inferred_max_damage(slot);
    if let Some(components) = slot.components_to_add.as_ref() {
        for component in components {
            match component {
                ComponentsToAdd::MinecraftDamage(value) => damage = value.damage.0,
                ComponentsToAdd::MinecraftMaxDamage(value) => max_damage = value.max_damage.0,
                _ => {}
            }
        }
    }
    (damage.max(0), max_damage.max(0))
}

pub(super) fn apply_damage_component(slot: &mut Slot, damage: i32, max_damage: i32) {
    if max_damage <= 0 {
        return;
    }
    let components = slot.components_to_add.get_or_insert_with(Vec::new);
    upsert_component(
        components,
        |component| matches!(component, ComponentsToAdd::MinecraftMaxDamage(_)),
        ComponentsToAdd::MinecraftMaxDamage(minecraft::MaxDamage {
            max_damage: VarInt(max_damage),
        }),
    );
    upsert_component(
        components,
        |component| matches!(component, ComponentsToAdd::MinecraftDamage(_)),
        ComponentsToAdd::MinecraftDamage(minecraft::Damage {
            damage: VarInt(damage.max(0)),
        }),
    );
    slot.number_of_components_to_add = Some(VarInt(components.len() as i32));
    slot.number_of_components_to_remove
        .get_or_insert_with(|| VarInt(0));
}

pub(super) fn is_unbreakable(slot: &Slot) -> bool {
    slot.components_to_add.as_ref().is_some_and(|components| {
        components
            .iter()
            .any(|component| matches!(component, ComponentsToAdd::MinecraftUnbreakable(_)))
    })
}

pub(super) fn decrement_slot(slot: &mut Slot, amount: i32) {
    let amount = amount.max(0);
    if amount == 0 || slot.item_count.0 <= 0 {
        return;
    }
    slot.item_count.0 = slot.item_count.0.saturating_sub(amount);
    if slot.item_count.0 <= 0 {
        *slot = crate::inventory::empty_slot();
    }
}

pub(super) fn same_item_name(slot: &Slot, name: &str) -> bool {
    let Some(item_id) = slot.item_id.as_ref().map(|id| id.0) else {
        return false;
    };
    crate::inventory::item_name_for_id(item_id).as_deref() == Some(name)
}

pub(super) fn item_name(slot: &Slot) -> String {
    slot.item_id
        .as_ref()
        .and_then(|id| crate::inventory::item_name_for_id(id.0))
        .unwrap_or_default()
}

fn upsert_component(
    components: &mut Vec<ComponentsToAdd>,
    matches: impl Fn(&ComponentsToAdd) -> bool,
    replacement: ComponentsToAdd,
) {
    if let Some(component) = components.iter_mut().find(|component| matches(component)) {
        *component = replacement;
    } else {
        components.push(replacement);
    }
}

fn inferred_max_damage(slot: &Slot) -> i32 {
    let name = item_name(slot);
    match name.as_str() {
        name if name.ends_with("_helmet") => material_durability(name, 55, 165, 363, 407, 77),
        name if name.ends_with("_chestplate") => material_durability(name, 80, 240, 528, 592, 112),
        name if name.ends_with("_leggings") => material_durability(name, 75, 225, 495, 555, 105),
        name if name.ends_with("_boots") => material_durability(name, 65, 195, 429, 481, 91),
        name if name.ends_with("_sword") => tool_durability(name),
        name if name.ends_with("_pickaxe") => tool_durability(name),
        name if name.ends_with("_axe") => tool_durability(name),
        name if name.ends_with("_shovel") => tool_durability(name),
        name if name.ends_with("_hoe") => tool_durability(name),
        "minecraft:shears" => 238,
        "minecraft:flint_and_steel" => 64,
        "minecraft:bow" => 384,
        "minecraft:crossbow" => 465,
        "minecraft:fishing_rod" => 64,
        "minecraft:trident" => 250,
        "minecraft:mace" => 500,
        _ => 0,
    }
}

fn tool_durability(name: &str) -> i32 {
    if name.contains("wooden_") {
        59
    } else if name.contains("stone_") {
        131
    } else if name.contains("iron_") {
        250
    } else if name.contains("golden_") {
        32
    } else if name.contains("diamond_") {
        1561
    } else if name.contains("netherite_") {
        2031
    } else {
        0
    }
}

fn material_durability(
    name: &str,
    leather: i32,
    chainmail_iron: i32,
    diamond: i32,
    netherite: i32,
    golden: i32,
) -> i32 {
    if name.contains("leather_") {
        leather
    } else if name.contains("chainmail_") || name.contains("iron_") {
        chainmail_iron
    } else if name.contains("diamond_") {
        diamond
    } else if name.contains("netherite_") {
        netherite
    } else if name.contains("golden_") {
        golden
    } else {
        0
    }
}

fn collect_plugin_enchantments(tag: &qexed_nbt::Tag, out: &mut Vec<PluginEnchantment>) {
    let qexed_nbt::Tag::Compound(root) = tag else {
        return;
    };
    let Some(enchantments) = root
        .get("qexed:enchantments")
        .or_else(|| root.get("qexed_enchantments"))
    else {
        return;
    };
    match enchantments {
        qexed_nbt::Tag::Compound(values) => {
            for (id, level) in values.iter() {
                if let Some(level) = tag_i32(level).filter(|level| *level > 0) {
                    out.push(PluginEnchantment {
                        id: id.clone(),
                        level,
                    });
                }
            }
        }
        qexed_nbt::Tag::List(_, values) => {
            for value in values.iter() {
                let qexed_nbt::Tag::Compound(entry) = value else {
                    continue;
                };
                let Some(qexed_nbt::Tag::String(id)) = entry.get("id") else {
                    continue;
                };
                let level = entry.get("level").and_then(tag_i32).unwrap_or(1);
                if level > 0 {
                    out.push(PluginEnchantment {
                        id: id.to_string(),
                        level,
                    });
                }
            }
        }
        _ => {}
    }
}

fn tag_i32(tag: &qexed_nbt::Tag) -> Option<i32> {
    match tag {
        qexed_nbt::Tag::Byte(value) => Some(i32::from(*value)),
        qexed_nbt::Tag::Short(value) => Some(i32::from(*value)),
        qexed_nbt::Tag::Int(value) => Some(*value),
        qexed_nbt::Tag::Long(value) => i32::try_from(*value).ok(),
        _ => None,
    }
}

fn enchantment_name(id: i32) -> &'static str {
    match id {
        0 => "minecraft:aqua_affinity",
        1 => "minecraft:bane_of_arthropods",
        2 => "minecraft:binding_curse",
        3 => "minecraft:blast_protection",
        4 => "minecraft:breach",
        5 => "minecraft:channeling",
        6 => "minecraft:density",
        7 => "minecraft:depth_strider",
        8 => "minecraft:efficiency",
        9 => "minecraft:feather_falling",
        10 => "minecraft:fire_aspect",
        11 => "minecraft:fire_protection",
        12 => "minecraft:flame",
        13 => "minecraft:fortune",
        14 => "minecraft:frost_walker",
        15 => "minecraft:impaling",
        16 => "minecraft:infinity",
        17 => "minecraft:knockback",
        18 => "minecraft:looting",
        19 => "minecraft:loyalty",
        20 => "minecraft:luck_of_the_sea",
        21 => "minecraft:lunge",
        22 => "minecraft:lure",
        23 => "minecraft:mending",
        24 => "minecraft:multishot",
        25 => "minecraft:piercing",
        26 => "minecraft:power",
        27 => "minecraft:projectile_protection",
        28 => "minecraft:protection",
        29 => "minecraft:punch",
        30 => "minecraft:quick_charge",
        31 => "minecraft:respiration",
        32 => "minecraft:riptide",
        33 => "minecraft:sharpness",
        34 => "minecraft:silk_touch",
        35 => "minecraft:smite",
        36 => "minecraft:soul_speed",
        37 => "minecraft:sweeping_edge",
        38 => "minecraft:swift_sneak",
        39 => "minecraft:thorns",
        40 => "minecraft:unbreaking",
        41 => "minecraft:vanishing_curse",
        42 => "minecraft:wind_burst",
        _ => "minecraft:unknown",
    }
}
