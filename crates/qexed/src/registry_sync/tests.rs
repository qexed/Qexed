use super::{
    STATIC_TAG_REGISTRIES, VANILLA_FEATURE, accepts_vanilla_core_pack, known_packs,
    load_registry_packets, load_tag_packet,
};

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
fn loads_tags_from_assets() {
    let packet = load_tag_packet().unwrap();
    assert!(
        packet
            .tags
            .iter()
            .any(|tags| tags.registry == "minecraft:damage_type")
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
