//! 迁移测试：真实 1.17 区块 -> 1.18+ 布局，方块数据语义等价。

use std::path::PathBuf;

use qexed_anvil::chunk::{self};
use qexed_anvil::migrate::{self, MigrateOptions, MigrationOutcome};
use qexed_nbt::Tag;

fn fixtures_dir() -> Option<PathBuf> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join(".tmp/anvil_fixtures");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn load(name: &str) -> Option<Tag> {
    let dir = fixtures_dir()?;
    let path = dir.join(name);
    if !path.exists() {
        return None;
    };
    let bytes = std::fs::read(path).unwrap();
    Some(qexed_nbt::from_slice_lossy(&bytes).unwrap().1)
}

fn all_block_refs(root: &Tag) -> Vec<(i32, Vec<chunk::BlockStateRef>, Vec<usize>)> {
    let mut v: Vec<_> = chunk::all_section_blocks(root)
        .unwrap()
        .into_iter()
        .map(|s| (s.section_y, s.palette, s.indices))
        .collect();
    v.sort_by_key(|(y, _, _)| *y);
    v
}

#[test]
fn migrates_1_17_chunk_to_modern_layout() {
    let Some(root) = load("1.17.1.chunk") else { return };
    let before = all_block_refs(&root);
    let (migrated, outcome) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    assert_eq!(outcome, MigrationOutcome::Migrated);
    let fields = chunk::root_compound(&migrated).unwrap();
    assert!(fields.get("Level").is_none());
    assert!(fields.get("sections").is_some());
    assert!(fields.get("yPos").is_some());
    match fields.get("Status") {
        Some(Tag::String(status)) => assert!(status.starts_with("minecraft:")),
        other => panic!("Status missing: {other:?}"),
    }
    assert_eq!(fields.get("DataVersion"), Some(&Tag::Int(3337)));
    let after = all_block_refs(&migrated);
    assert_eq!(before, after, "block data preserved");
}

#[test]
fn migrated_chunk_reads_with_modern_api_and_writes_back() {
    let Some(root) = load("1.17.1.chunk") else { return };
    let (migrated, _) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    let state = chunk::block_state_at(&migrated, 8, 40, 8).unwrap();
    assert!(state.is_some());
    let target = chunk::BlockStateRef::new("minecraft:diamond_block");
    let edited = chunk::set_block_state(
        &migrated, 8, 40, 8, &target,
        &chunk::BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    ).unwrap();
    assert_eq!(chunk::block_state_at(&edited, 8, 40, 8).unwrap().unwrap(), target);
    let bytes = qexed_nbt::to_vec("", &edited).unwrap();
    let back = qexed_nbt::from_slice(&bytes).unwrap().1;
    assert_eq!(back, edited);
}

#[test]
fn modern_chunk_passes_through() {
    let Some(root) = load("issue99-chunk.nbt") else { return };
    let (out, outcome) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    assert_eq!(outcome, MigrationOutcome::AlreadyModern);
    assert_eq!(out, root);
}

#[test]
fn pre_flattening_chunk_upgrades_through_pipeline() {
    let Some(root) = load("1.12.chunk") else { return };
    let (upgraded, outcome) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    assert_eq!(outcome, MigrationOutcome::MigratedPreFlattening);
    let fields = chunk::root_compound(&upgraded).unwrap();
    assert!(fields.get("Level").is_none());
    assert!(fields.get("sections").is_some());
    assert_eq!(fields.get("DataVersion"), Some(&Tag::Int(3337)));
    let sections = chunk::all_section_blocks(&upgraded).unwrap();
    assert_eq!(sections.len(), 5);
}
#[test]
fn migrated_sections_carry_named_biomes() {
    let Some(root) = load("1.17.1.chunk") else { return };
    let (migrated, _) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    let fields = chunk::root_compound(&migrated).unwrap();
    let Tag::List(_, sections) = fields.get("sections").unwrap() else { panic!() };
    let mut biome_named = 0;
    for section in sections.iter() {
        let Tag::Compound(section_map) = section else { continue };
        let Some(Tag::Compound(biomes)) = section_map.get("biomes") else { continue };
        let Some(Tag::List(_, palette)) = biomes.get("palette") else { continue };
        for entry in palette.iter() {
            if let Tag::String(name) = entry {
                assert!(name.contains(':'), "namespaced: {name}");
                biome_named += 1;
            }
        }
    }
    assert!(biome_named > 0);
}

#[test]
fn migrate_all_legacy_fixture_chunks() {
    for name in [
        "1.17.0.chunk", "1.17.1-custom-heights.chunk", "1.17.1.chunk",
        "etho.chunk", "etho-max-heights.chunk", "etho-old-heightmaps.chunk",
        "etho-old-in-new.chunk", "etho-old-in-new2.chunk", "unicode.chunk",
    ] {
        let Some(root) = load(name) else { continue };
        let before = all_block_refs(&root);
        let (migrated, outcome) = migrate::migrate_chunk(&root, &MigrateOptions::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(outcome, MigrationOutcome::Migrated, "{name}");
        let after = all_block_refs(&migrated);
        assert_eq!(before.len(), after.len(), "{name}: section count");
        assert_eq!(before, after, "{name}: block data preserved");
    }
}

#[test]
fn migrated_chunk_uses_modern_field_names() {
    let Some(root) = load("1.17.1.chunk") else { return };
    let (migrated, _) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    let fields = chunk::root_compound(&migrated).unwrap();
    for key in ["block_entities", "block_ticks", "fluid_ticks"] {
        assert!(fields.contains_key(key), "missing {key}");
    }
    for key in ["TileEntities", "TileTicks", "LiquidTicks", "Level"] {
        assert!(!fields.contains_key(key), "stale {key}");
    }
}

#[test]
fn custom_height_world_migrates_with_correct_biomes() {
    let Some(root) = load("1.17.1-custom-heights.chunk") else { return };
    let (migrated, outcome) = migrate::migrate_chunk(&root, &MigrateOptions::default()).unwrap();
    assert_eq!(outcome, MigrationOutcome::Migrated);
    let fields = chunk::root_compound(&migrated).unwrap();
    assert_eq!(fields.get("yPos"), Some(&Tag::Int(-9)));
    let Tag::List(_, sections) = fields.get("sections").unwrap() else { panic!() };
    assert!(!sections.is_empty());
    for section in sections.iter() {
        let Tag::Compound(section_map) = section else { continue };
        let Some(Tag::Compound(biomes)) = section_map.get("biomes") else {
            panic!("section missing biomes");
        };
        let Some(Tag::List(_, palette)) = biomes.get("palette") else { panic!() };
        let Tag::String(name) = palette.first().unwrap() else { panic!() };
        assert_eq!(&**name, "minecraft:snowy_taiga", "biome 19 mapped");
    }
}
