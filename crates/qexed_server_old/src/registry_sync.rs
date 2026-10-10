//! 注册表同步：从 qexed_mojang_data 缓存构造 configuration 阶段的 registry/tags 包。
//!
//! 数据源（全部来自 `qexed_mojang_data` 的缓存，不再读取仓库 assets）：
//! - 动态注册表内容/tags：`cache/mojang/<ver>/data/minecraft`
//! - 静态注册表 protocol_id 映射：`cache/mojang/<ver>/reports/registries.json`

mod registries;
mod tags;
mod util;

pub use registries::{
    load_blocks_report, load_dynamic_registry_id_map, load_registry_id_map, load_registry_packets,
};
pub use tags::load_tag_packet;

use qexed_protocol::types::KnownPacks;

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
];

pub fn known_packs() -> Vec<KnownPacks> {
    vec![KnownPacks {
        namespace: "minecraft".to_string(),
        id: "core".to_string(),
        version: qexed_mojang_data::MC_VERSION.to_string(),
    }]
}

pub fn accepts_vanilla_core_pack(packs: &[KnownPacks]) -> bool {
    let known = known_packs();
    packs.iter().any(|pack| known.contains(pack))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{accepts_vanilla_core_pack, known_packs, load_registry_packets, load_tag_packet};

    /// 测试数据源：workspace 的 run/ 缓存（与主程序一致），缺失时现场初始化。
    fn ensure_cache() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mojang_cache = manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(|root| root.join("run").join("cache").join("mojang"))
            .expect("workspace root");
        let _ = qexed_mojang_data::set_cache_dir(&mojang_cache);
        if qexed_mojang_data::data_dir().is_err() || qexed_mojang_data::reports_dir().is_err() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(qexed_mojang_data::init()).expect("mojang data init");
        }
    }

    #[test]
    fn loads_vanilla_registry_packets_from_mojang_cache() {
        ensure_cache();
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
        ensure_cache();
        let packets = load_registry_packets(false).unwrap();
        let biome = packets
            .iter()
            .find(|packet| packet.id == "minecraft:worldgen/biome")
            .unwrap();

        assert!(!biome.entries.is_empty());
        assert!(biome.entries.iter().all(|entry| entry.data.is_none()));
    }

    #[test]
    fn loads_tags_from_mojang_cache() {
        ensure_cache();
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
            .expect("minecraft:damage_type/minecraft:is_fire is required by client item component initialization");
        assert!(
            !fire_tag.entries.is_empty(),
            "minecraft:damage_type/minecraft:is_fire must resolve to damage type ids"
        );
    }

    #[test]
    fn known_packs_negotiation() {
        let packs = known_packs();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].namespace, "minecraft");
        assert!(accepts_vanilla_core_pack(&packs));
        assert!(!accepts_vanilla_core_pack(&[]));
    }
}
