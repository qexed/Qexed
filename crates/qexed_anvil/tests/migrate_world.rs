//! 存档目录级迁移测试：真实 go-mc 世界的布局探测、幂等、完整旧版世界迁移。

use std::path::PathBuf;

use qexed_anvil::migrate::MigrateOptions;
use qexed_anvil::world::{self, WorldLayout};

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

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap();
        }
    }
}

#[test]
fn probe_detects_real_world_layout() {
    let Some(dir) = fixtures_dir() else { return };
    let w = dir.join("goworld");
    if !w.exists() { return };

    let layout: WorldLayout = world::probe(&w).unwrap();
    assert!(layout.has_level_dat);
    assert_eq!(layout.data_version, Some(2865));
    assert_eq!(layout.dimensions.len(), 1);
    assert_eq!(layout.dimensions[0].0, "minecraft:overworld");
    assert!(layout.dimensions[0].1.ends_with("region"));
    assert_eq!(layout.entities_dirs.len(), 1);
    assert_eq!(layout.poi_dirs.len(), 1);
    assert!(layout.legacy_mcr_files.is_empty());
    assert!(!layout.needs_migration());
}

#[test]
fn migrate_world_on_modern_world_updates_level_dat_only() {
    let Some(dir) = fixtures_dir() else { return };
    let w = dir.join("goworld");
    if !w.exists() { return };

    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("world");
    copy_dir(&w, &copy);

    let report = world::migrate_world(&copy, &MigrateOptions::default()).unwrap();
    let migrated: usize = report.dimensions.iter().map(|(_, s)| s.migrated).sum();
    assert_eq!(migrated, 0, "modern chunks untouched");

    // 目标版本(3337) > 当前(2865)：level.dat 应被更新且字段保留
    assert!(report.level_dat_updated);
    let (_, root) = qexed_nbt::from_file(copy.join("level.dat")).unwrap();
    let qexed_nbt::Tag::Compound(fields) = &root else { panic!() };
    let qexed_nbt::Tag::Compound(data) = fields.get("Data").unwrap() else { panic!() };
    assert_eq!(data.get("DataVersion"), Some(&qexed_nbt::Tag::Int(3337)));
    assert!(data.contains_key("LevelName") || data.contains_key("SpawnX"), "other fields kept");
}

#[test]
fn migrate_world_full_legacy_flow() {
    let Some(dir) = fixtures_dir() else { return };
    let chunk_path = dir.join("1.17.1.chunk");
    let level_src = dir.join("goworld/level.dat");
    if !chunk_path.exists() || !level_src.exists() { return };

    let tmp = tempfile::tempdir().unwrap();
    let w = tmp.path().join("world");
    let region_dir = w.join("region");
    std::fs::create_dir_all(&region_dir).unwrap();

    // level.dat -> DataVersion 2730 (1.17)
    let (_, root) = qexed_nbt::from_file(&level_src).unwrap();
    let qexed_nbt::Tag::Compound(fields_arc) = &root else { panic!() };
    let mut fields = fields_arc.as_ref().clone();
    if let Some(qexed_nbt::Tag::Compound(data)) = fields.get_mut("Data") {
        std::sync::Arc::make_mut(data).insert(
            "DataVersion".to_string(),
            qexed_nbt::Tag::Int(2730),
        );
    }
    let root = qexed_nbt::Tag::Compound(std::sync::Arc::new(fields));
    qexed_nbt::to_file(w.join("level.dat"), "", &root, true).unwrap();

    // region/(1.17 区块)
    let chunk_bytes = std::fs::read(&chunk_path).unwrap();
    let chunk_root = qexed_nbt::from_slice_lossy(&chunk_bytes).unwrap().1;
    let serialized = qexed_nbt::to_vec("", &chunk_root).unwrap();
    {
        use qexed_anvil::region::{AnvilRegion, ChunkData};
        let mut region = AnvilRegion::new(region_dir.join("r.0.0.mca"));
        region.write_chunk(0, 0, ChunkData::zlib(&serialized).unwrap()).unwrap();
        region.save().unwrap();
    }

    let layout = world::probe(&w).unwrap();
    assert!(layout.needs_migration());

    let report = world::migrate_world(&w, &MigrateOptions::default()).unwrap();
    assert_eq!(report.dimensions.len(), 1);
    assert_eq!(report.dimensions[0].1.migrated, 1);
    assert_eq!(report.dimensions[0].1.failed, 0);
    assert!(report.level_dat_updated);
    assert_eq!(report.skipped_mcr, 0);

    let layout = world::probe(&w).unwrap();
    assert!(!layout.needs_migration());

    use qexed_anvil::region::AnvilRegion;
    let region = AnvilRegion::from_file(region_dir.join("r.0.0.mca")).unwrap();
    let data = region.read_chunk(0, 0).unwrap().unwrap();
    let raw = data.decompress().unwrap();
    let (_, root) = qexed_nbt::from_slice(&raw).unwrap();
    let fields = qexed_anvil::chunk::root_compound(&root).unwrap();
    assert!(fields.get("Level").is_none());
    assert!(fields.get("sections").is_some());
}
