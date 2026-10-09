//! 区域文件级迁移端到端：真实 1.17 区块打包成 .mca -> 迁移 -> 全部现代化。

use std::path::PathBuf;

use qexed_anvil::chunk::{self};
use qexed_anvil::migrate::{self, MigrateOptions, MigrationOutcome, MigrationStats};
use qexed_anvil::region::{AnvilRegion, ChunkData};

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

fn load(name: &str) -> Option<qexed_nbt::Tag> {
    let dir = fixtures_dir()?;
    let path = dir.join(name);
    if !path.exists() {
        return None;
    };
    let bytes = std::fs::read(path).unwrap();
    Some(qexed_nbt::from_slice_lossy(&bytes).unwrap().1)
}

fn block_fingerprint(root: &qexed_nbt::Tag) -> Vec<(i32, Vec<chunk::BlockStateRef>, Vec<usize>)> {
    let mut v: Vec<_> = chunk::all_section_blocks(root)
        .unwrap()
        .into_iter()
        .map(|s| (s.section_y, s.palette, s.indices))
        .collect();
    v.sort_by_key(|(y, _, _)| *y);
    v
}

#[test]
fn region_file_migration_end_to_end() {
    // 1) 用真实 1.17 区块构造一个旧版区域文件
    let sources = [
        "1.17.1.chunk",
        "1.17.0.chunk",
        "etho.chunk",
        "unicode.chunk",
    ];
    let roots: Vec<_> = sources.iter().filter_map(|n| load(n)).collect();
    if roots.is_empty() { return };

    // 迁移前指纹（含每区块坐标假设：lx = index）
    let fingerprints: Vec<_> = roots.iter().map(|r| block_fingerprint(r)).collect();

    let tmp = tempfile::tempdir().unwrap();
    let mca = tmp.path().join("r.0.0.mca");
    {
        let mut region = AnvilRegion::new(&mca);
        for (index, root) in roots.iter().enumerate() {
            let lx = (index % 4) as i32;
            let lz = (index / 4) as i32;
            let bytes = qexed_nbt::to_vec("", root).unwrap();
            region.write_chunk(lx, lz, ChunkData::zlib(&bytes).unwrap()).unwrap();
        }
        region.save().unwrap();
    }

    // 2) 迁移
    let stats = migrate::migrate_region_file(&mca, &MigrateOptions::default()).unwrap();
    assert_eq!(
        stats,
        MigrationStats { migrated: roots.len(), modern: 0, unsupported: 0, failed: 0 },
        "all chunks migrated",
    );

    // 3) 读回验证：全部现代布局 + 方块指纹一致
    let region = AnvilRegion::from_file(&mca).unwrap();
    for (index, expected) in fingerprints.iter().enumerate() {
        let lx = (index % 4) as i32;
        let lz = (index / 4) as i32;
        let data = region.read_chunk(lx, lz).unwrap().expect("chunk exists");
        let raw = data.decompress().unwrap();
        let (_, root) = qexed_nbt::from_slice(&raw).unwrap();

        let fields = chunk::root_compound(&root).unwrap();
        assert!(fields.get("Level").is_none(), "chunk {index} modern");
        assert!(fields.get("sections").is_some());

        assert_eq!(&block_fingerprint(&root), expected, "chunk {index} blocks preserved");
    }

    // 4) 二次迁移：全部 AlreadyModern，文件不再变化
    let stats2 = migrate::migrate_region_file(&mca, &MigrateOptions::default()).unwrap();
    assert_eq!(stats2.migrated, 0);
    assert_eq!(stats2.modern, roots.len());

    let (_, outcome) = migrate::migrate_chunk(
        &{
            let data = region.read_chunk(0, 0).unwrap().unwrap();
            let raw = data.decompress().unwrap();
            qexed_nbt::from_slice(&raw).unwrap().1
        },
        &MigrateOptions::default(),
    )
    .unwrap();
    assert_eq!(outcome, MigrationOutcome::AlreadyModern);
}
