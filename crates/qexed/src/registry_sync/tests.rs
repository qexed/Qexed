use super::{
    STATIC_TAG_REGISTRIES, VANILLA_FEATURE, accepts_vanilla_core_pack, known_packs,
    load_registry_packets, load_tag_packet,
};
use qexed_nbt::Tag;

#[test]
fn loads_vanilla_registry_packets_from_assets() {
    let packets = load_registry_packets(true).unwrap();
    assert!(
        packets
            .iter()
            .any(|packet| packet.id == "minecraft:worldgen/biome")
    );
    assert!(
        packets
            .iter()
            .any(|packet| packet.id == "minecraft:dimension_type")
    );
}

#[test]
fn can_skip_vanilla_registry_contents_for_known_pack_clients() {
    let packets = load_registry_packets(false).unwrap();
    let biome = packets
        .iter()
        .find(|packet| packet.id == "minecraft:worldgen/biome")
        .unwrap();

    assert!(!biome.entries.is_empty());
    assert!(biome.entries.iter().all(|entry| entry.data.is_none()));
}

#[test]
fn full_registry_contents_encode_json_decimals_as_nbt_float() {
    let packets = load_registry_packets(true).unwrap();
    let enchantments = packets
        .iter()
        .find(|packet| packet.id == "minecraft:enchantment")
        .expect("minecraft:enchantment registry must be synchronized");
    let depth_strider = enchantments
        .entries
        .iter()
        .find(|entry| entry.entry_id == "minecraft:depth_strider")
        .expect("minecraft:depth_strider enchantment must be present");

    let data = depth_strider
        .data
        .as_ref()
        .expect("full registry contents must include enchantment data");
    let Some(Tag::Float(value)) = nested_tag(
        data,
        &["effects", "minecraft:attributes", "0", "amount", "base"],
    ) else {
        panic!("decimal enchantment amount must be encoded as NBT float");
    };

    assert!((*value - 0.33333334).abs() < f32::EPSILON);
}

#[test]
fn loads_tags_from_assets() {
    let packet = load_tag_packet().unwrap();
    let damage_type_tags = packet
        .tags
        .iter()
        .find(|tags| tags.registry == "minecraft:damage_type")
        .expect("minecraft:damage_type tags must be synchronized");
    let fire_tag = damage_type_tags
        .tags
        .iter()
        .find(|tag| tag.name == "minecraft:is_fire")
        .expect(
            "minecraft:damage_type/minecraft:is_fire is required by client item component initialization",
        );
    assert!(
        !fire_tag.entries.is_empty(),
        "minecraft:damage_type/minecraft:is_fire must resolve to damage type ids"
    );
    let item_tags = packet
        .tags
        .iter()
        .find(|tags| tags.registry == "minecraft:item")
        .expect("minecraft:item tags are required for enchantment registry loading");
    let enchantable_head_armor = item_tags
        .tags
        .iter()
        .find(|tag| tag.name == "minecraft:enchantable/head_armor")
        .expect("minecraft:item/minecraft:enchantable/head_armor must be synchronized");
    assert!(
        !enchantable_head_armor.entries.is_empty(),
        "minecraft:item/minecraft:enchantable/head_armor must resolve to item protocol ids"
    );
    assert!(STATIC_TAG_REGISTRIES.iter().any(|registry| {
        packet
            .tags
            .iter()
            .any(|tags| tags.registry == format!("minecraft:{registry}"))
    }));
}

#[test]
fn known_pack_matches_vanilla_core_pack() {
    let packs = known_packs();
    assert_eq!(VANILLA_FEATURE, "minecraft:vanilla");
    assert_eq!(packs[0].namespace, "minecraft");
    assert_eq!(packs[0].id, "core");
    assert!(accepts_vanilla_core_pack(&packs));
}

fn nested_tag<'a>(tag: &'a Tag, path: &[&str]) -> Option<&'a Tag> {
    let mut current = tag;
    for segment in path {
        current = match current {
            Tag::Compound(map) => map.get(*segment)?,
            Tag::List(_, items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}
