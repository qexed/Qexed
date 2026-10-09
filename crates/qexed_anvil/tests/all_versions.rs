//! 全版本真实存档回归测试（mc-worldgen/mc-saves，1.12.2 → 26.3）。
//! fixture 根由 MC_SAVES_DIR 环境变量或默认路径提供；不存在时跳过。

use std::path::{Path, PathBuf};

use qexed_anvil::chunk;
use qexed_anvil::region::AnvilRegion;
use qexed_anvil::world::{self, WorldLayout};

fn saves_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("MC_SAVES_DIR") {
        let dir = PathBuf::from(dir);
        return dir.is_dir().then_some(dir);
    }
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("mc-worldgen/mc-saves");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn version_key(name: &str) -> Vec<u64> {
    name.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn list_versions() -> Vec<(String, PathBuf)> {
    let Some(root) = saves_dir() else { return Vec::new() };
    let mut versions: Vec<(String, PathBuf)> = std::fs::read_dir(&root)
        .into_iter().flatten().flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
        .collect();
    versions.sort_by(|a, b| version_key(&a.0).cmp(&version_key(&b.0)));
    versions
}

fn region_mcas(world: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![world.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "entities" || name == "poi" { continue }
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("mca") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn all_version_saves_parse() {
    let versions = list_versions();
    if versions.is_empty() {
        eprintln!("mc-saves fixture not found, skipping");
        return;
    }
    assert!(versions.len() >= 40, "expected ~48 versions, got {}", versions.len());

    let mut total_chunks = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for (name, world) in &versions {
        let files = region_mcas(world);
        if files.is_empty() {
            eprintln!("{name}: (no region files)");
            continue;
        }

        let (mut chunks, mut sections, mut blocks) = (0usize, 0usize, 0usize);
        let mut version_failures = Vec::new();

        for file in &files {
            let Ok(region) = AnvilRegion::from_file(file) else {
                version_failures.push(format!("open failed: {}", file.display()));
                continue;
            };
            for (lx, lz) in region.chunk_coords() {
                let Ok(Some(data)) = region.read_chunk(lx, lz) else { continue };
                chunks += 1;
                let Ok(raw) = data.decompress() else {
                    version_failures.push(format!("decompress failed ({lx},{lz})"));
                    continue;
                };
                let Ok((_, root)) = qexed_nbt::from_slice_lossy(&raw) else {
                    version_failures.push(format!("nbt parse failed ({lx},{lz})"));
                    continue;
                };
                if let Ok(secs) = chunk::all_section_blocks(&root) {
                    sections += secs.len();
                    for s in &secs { blocks += s.indices.len(); }
                }
            }
        }

        eprintln!("{name}: {} files, {} chunks, {} sections, {} blocks{}",
            files.len(), chunks, sections, blocks,
            if version_failures.is_empty() { String::new() }
            else { format!(" !! {} FAILURES", version_failures.len()) });
        failures.extend(version_failures.into_iter().map(|f| format!("{name}: {f}")));
        total_chunks += chunks;
    }

    assert!(total_chunks > 1000, "too few total chunks: {total_chunks}");
    assert!(failures.is_empty(), "failures: {:#?}", &failures[..failures.len().min(10)]);
}

#[test]
fn world_probe_recognizes_all_layouts() {
    let versions = list_versions();
    if versions.is_empty() { return }

    for (name, world) in &versions {
        let layout: WorldLayout = match world::probe(world) {
            Ok(layout) => layout,
            Err(e) => panic!("{name}: probe failed: {e}"),
        };
        assert!(layout.has_level_dat, "{name}: level.dat missing");
        assert!(
            layout.dimensions.iter().any(|(d, _)| d == "minecraft:overworld"),
            "{name}: overworld not detected (dims: {:?})",
            layout.dimensions.iter().map(|(d, _)| d.as_str()).collect::<Vec<_>>(),
        );
        if let Some(v) = layout.data_version {
            assert!(v > 0 && v < 100000, "{name}: weird DataVersion {v}");
        }
    }
}

#[test]
fn migration_end_to_end_pre_1_18() {
    let versions = list_versions();
    if versions.is_empty() { return }

    for (name, world) in &versions {
        let Some(dv) = world::probe(world).ok().and_then(|l| l.data_version) else { continue };
        if dv >= 2860 { continue }
        if dv < 1519 { continue }

        let tmp = tempfile::tempdir().unwrap();
        let copy = tmp.path().join("world");
        copy_region_only(world, &copy);

        let report = world::migrate_world(&copy, &qexed_anvil::migrate::MigrateOptions::default())
            .unwrap_or_else(|e| panic!("{name}: migrate failed: {e}"));

        let failed: usize = report.dimensions.iter().map(|(_, s)| s.failed).sum();
        assert_eq!(failed, 0, "{name}: {failed} chunks failed");

        let layout = world::probe(&copy).unwrap();
        assert!(!layout.needs_migration(), "{name}: still needs migration");

        let migrated: usize = report.dimensions.iter().map(|(_, s)| s.migrated).sum();
        assert!(migrated > 0, "{name}: nothing migrated");
        eprintln!("{name}: migrated {migrated} chunks");
    }
}

#[test]
fn upgrade_1_12_world_end_to_end() {
    let Some(dir) = saves_dir() else { return };
    let world = dir.join("1.12.2");
    if !world.is_dir() { return }

    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("world");
    copy_region_only(&world, &copy);

    let report = world::migrate_world(&copy, &qexed_anvil::migrate::MigrateOptions::default()).unwrap();
    let migrated: usize = report.dimensions.iter().map(|(_, s)| s.migrated).sum();
    let failed: usize = report.dimensions.iter().map(|(_, s)| s.failed).sum();
    assert_eq!(failed, 0);
    assert!(migrated > 0, "1.12.2 world should have flattened chunks");
    eprintln!("1.12.2: flattened {migrated} chunks");
}

fn copy_region_only(src: &Path, dst: &Path) {
    if src.join("level.dat").is_file() {
        std::fs::create_dir_all(dst).unwrap();
        std::fs::copy(src.join("level.dat"), dst.join("level.dat")).unwrap();
    }
    fn copy_with_region(src: &Path, dst: &Path) {
        let Ok(entries) = std::fs::read_dir(src) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !path.is_dir() { continue }
            if name == "entities" || name == "poi" || name == "playerdata" || name == "players" { continue }
            if name == "region" {
                let target = dst.join(&name);
                std::fs::create_dir_all(&target).unwrap();
                for f in std::fs::read_dir(&path).unwrap().flatten() {
                    if f.path().extension().and_then(|e| e.to_str()) == Some("mca") {
                        std::fs::copy(f.path(), target.join(f.file_name())).unwrap();
                    }
                }
            } else {
                copy_with_region(&path, &dst.join(&name));
            }
        }
    }
    copy_with_region(src, dst);
}
