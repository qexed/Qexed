use qexed_protocol::types::KnownPacks;
use std::path::PathBuf;

mod registries;
mod tags;
mod util;

pub use registries::load_registry_packets;
pub use registries::{
    load_blocks_report, load_dynamic_registry_id_map, load_registry_id_map,
};
pub use tags::load_tag_packet;

pub const VANILLA_FEATURE: &str = "minecraft:vanilla";

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
    // 26.3: item 组件初始化（shovel/axe/hoe）所需
    "block_transformer",
    "decorated_pot_pattern",
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

pub fn ensure_data_ready() -> Result<()> {
    let roots = data_roots();
    if roots
        .iter()
        .any(|root| data_root_has_required_registries(root))
    {
        return Ok(());
    }
    Err(RegistryError::msg(
        "vanilla registry data is unavailable; run qexed_mojang_data::init() first",
    ))
}

pub(crate) fn lang_dir() -> Option<PathBuf> {
    for root in data_roots() {
        let lang_dir = root.join("lang");
        if lang_dir.is_dir() {
            return Some(lang_dir);
        }
    }
    None
}

fn data_roots() -> Vec<PathBuf> {
    let version = qexed_config::MC_VERSION;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![
        cwd.join("cache/mojang").join(version).join("data/minecraft"),
        cwd.join("assets/vanilla_json/minecraft"),
    ]
}

fn data_root_has_required_registries(root: &std::path::Path) -> bool {
    root.join("dimension_type").is_dir()
        && root.join("damage_type").is_dir()
        && root.join("tags/damage_type").is_dir()
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("nbt error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),
    #[error("{0}")]
    Message(String),
    #[error("path prefix: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),
}

impl RegistryError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, RegistryError>;

#[cfg(test)]
mod tests;
