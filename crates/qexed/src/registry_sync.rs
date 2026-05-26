use qexed_protocol::types::KnownPacks;

mod registries;
mod tags;
mod util;

pub use registries::load_registry_packets;
pub use tags::load_tag_packet;

pub const VANILLA_FEATURE: &str = "minecraft:vanilla";

const DATA_ROOT: &str = "assets/decompiled_source/src/data/minecraft";
const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

const STATIC_TAG_REGISTRIES: &[&str] = &[
    "block",
    "entity_type",
    "fluid",
    "game_event",
    "item",
    "point_of_interest_type",
    "potion",
];

const SYNCHRONIZED_REGISTRIES: &[&str] = &[
    "worldgen/biome",
    "chat_type",
    "trim_pattern",
    "trim_material",
    "wolf_variant",
    "wolf_sound_variant",
    "pig_variant",
    "pig_sound_variant",
    "frog_variant",
    "cat_variant",
    "cat_sound_variant",
    "cow_sound_variant",
    "cow_variant",
    "chicken_sound_variant",
    "chicken_variant",
    "zombie_nautilus_variant",
    "painting_variant",
    "dimension_type",
    "damage_type",
    "banner_pattern",
    "enchantment",
    "jukebox_song",
    "instrument",
    "test_environment",
    "test_instance",
    "dialog",
    "world_clock",
    "timeline",
];

pub fn known_packs() -> Vec<KnownPacks> {
    vec![KnownPacks {
        namespace: "minecraft".to_string(),
        id: "core".to_string(),
        version: qexed_config::MC_VERSION.to_string(),
    }]
}

pub fn accepts_vanilla_core_pack(packs: &[KnownPacks]) -> bool {
    let known = known_packs();
    packs.iter().any(|pack| known.contains(pack))
}

#[cfg(test)]
mod tests;
