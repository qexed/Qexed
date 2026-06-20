use std::path::{Path, PathBuf};

use qexed_protocol::types::KnownPacks;

use super::mojang;

pub const VANILLA_FEATURE: &str = "minecraft:vanilla";
pub(crate) const REGISTRIES_REPORT: &str = "registries.json";
pub(crate) const BLOCKS_REPORT: &str = "blocks.json";

pub const STATIC_TAG_REGISTRIES: &[&str] = &[
    "block",
    "entity_type",
    "fluid",
    "game_event",
    "item",
    "point_of_interest_type",
    "potion",
];

pub(crate) const SYNCHRONIZED_REGISTRIES: &[&str] = &[
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

pub(crate) fn ensure_data_ready() -> anyhow::Result<()> {
    let roots = data_roots();
    if roots
        .iter()
        .any(|root| data_root_has_required_registries(root))
    {
        return Ok(());
    }

    anyhow::bail!(
        "vanilla registry data is unavailable; Mojang registry data has not been initialized"
    )
}

#[allow(dead_code)]
pub(crate) fn lang_dir() -> Option<PathBuf> {
    data_roots()
        .into_iter()
        .map(|root| root.join("lang"))
        .find(|path| path.is_dir())
}

pub(crate) fn data_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(path) = mojang::data_root() {
        roots.push(path);
    }

    roots
}

fn data_root_has_required_registries(root: &Path) -> bool {
    root.join("dimension_type").is_dir()
        && root.join("damage_type").is_dir()
        && root.join("tags/damage_type").is_dir()
}
