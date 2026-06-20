use qexed_protocol::types::KnownPacks;
use std::{path::PathBuf, sync::OnceLock};

mod mojang;
mod registries;
mod tags;
mod util;

pub use registries::load_registry_packets;
pub(crate) use registries::{
    load_blocks_report, load_dynamic_registry_id_map, load_registry_id_map,
};
pub use tags::load_tag_packet;

pub const VANILLA_FEATURE: &str = "minecraft:vanilla";

const DATA_ROOT: &str = "assets/decompiled_source/src/data/minecraft";
const VANILLA_JSON_ROOT: &str = "assets/vanilla_json/minecraft";
const REGISTRIES_REPORT: &str = "assets/reports/registries.json";
const BLOCKS_REPORT: &str = "assets/reports/blocks.json";

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

static MOJANG_CACHE_PATH: OnceLock<PathBuf> = OnceLock::new();

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
        "vanilla registry data is unavailable; configure network access or provide local vanilla data assets"
    );
}

/// Returns the path to the cached Mojang language files directory, if available.
/// This will trigger data download if not already cached.
pub(crate) fn lang_dir() -> Option<std::path::PathBuf> {
    let roots = data_roots();
    for root in &roots {
        let lang_dir = root.join("lang");
        if lang_dir.exists() && lang_dir.is_dir() {
            return Some(lang_dir);
        }
    }
    None
}

pub(crate) fn configure_mojang_cache_path(path: impl Into<PathBuf>) {
    let path = path.into();
    if path.as_os_str().is_empty() {
        return;
    }

    if MOJANG_CACHE_PATH.set(path).is_err() {
        log::debug!("Mojang registry cache path was already configured");
    }
}

pub(super) fn configured_mojang_cache_path() -> Option<PathBuf> {
    MOJANG_CACHE_PATH.get().cloned()
}

fn data_roots() -> Vec<PathBuf> {
    let root = util::workspace_root();
    let mut roots = vec![root.join(DATA_ROOT), root.join(VANILLA_JSON_ROOT)];

    if roots
        .iter()
        .any(|root| data_root_has_required_registries(root))
    {
        return roots;
    }

    match mojang_data_root() {
        Ok(path) => roots.insert(0, path),
        Err(err) => {
            log::warn!("Mojang registry data is unavailable, using local assets only: {err}")
        }
    }

    roots
}

fn data_root_has_required_registries(root: &std::path::Path) -> bool {
    root.join("dimension_type").is_dir()
        && root.join("damage_type").is_dir()
        && root.join("tags/damage_type").is_dir()
}

fn mojang_data_root() -> Result<PathBuf, String> {
    static ROOT: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        if tokio::runtime::Handle::try_current().is_ok() {
            return std::thread::spawn(|| mojang::data_root().map_err(|err| format!("{err:#}")))
                .join()
                .unwrap_or_else(|_| {
                    Err("Mojang registry data download thread panicked".to_string())
                });
        }

        mojang::data_root().map_err(|err| format!("{err:#}"))
    })
    .clone()
}

#[cfg(test)]
mod tests;
