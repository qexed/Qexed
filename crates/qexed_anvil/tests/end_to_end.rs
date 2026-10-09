//! 端到端：构造区块 NBT -> 压缩 -> 写入 .mca -> 读回 -> 解析 NBT -> 读方块。

use std::collections::HashMap;
use std::sync::Arc;

use qexed_anvil::chunk::{self, BlockStateRef, empty_section};
use qexed_anvil::region::AnvilRegion;
use qexed_nbt::{ListHeader, Tag, tag_id};

fn chunk_root(chunk_x: i32, chunk_z: i32) -> Tag {
    let section = empty_section(
        -4,
        &BlockStateRef::new("minecraft:deepslate"),
        "minecraft:plains",
    );
    let sections = Tag::List(
        ListHeader { tag_id: tag_id::COMPOUND, length: 1 },
        Arc::from(vec![Tag::Compound(Arc::new(section))]),
    );
    let mut root = HashMap::new();
    root.insert("DataVersion".to_string(), Tag::Int(4790));
    root.insert("xPos".to_string(), Tag::Int(chunk_x));
    root.insert("zPos".to_string(), Tag::Int(chunk_z));
    root.insert("Status".to_string(), Tag::String(Arc::from("minecraft:full")));
    root.insert("sections".to_string(), sections);
    Tag::Compound(Arc::new(root))
}

#[test]
fn region_nbt_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.0.0.mca");

    // 写入：NBT -> zlib -> region
    let root = chunk_root(0, 0);
    let updated = chunk::set_block_state(
        &root,
        8,
        -60,
        8,
        &BlockStateRef::new("minecraft:ancient_debris"),
        &BlockStateRef::new("minecraft:deepslate"),
        "minecraft:plains",
    )
    .unwrap();
    let bytes = qexed_nbt::to_vec("", &updated).unwrap();
    let chunk_data = qexed_anvil::region::ChunkData::zlib(&bytes).unwrap();

    let mut region = AnvilRegion::new(&path);
    region.write_chunk(0, 0, chunk_data).unwrap();
    region.save().unwrap();

    // 读回：region -> 解压 -> NBT -> 方块
    let region = AnvilRegion::from_file(&path).unwrap();
    let data = region.read_chunk(0, 0).unwrap().expect("chunk exists");
    let raw = data.decompress().unwrap();
    let (_, root_back) = qexed_nbt::from_slice(&raw).unwrap();
    let state = chunk::block_state_at(&root_back, 8, -60, 8)
        .unwrap()
        .expect("block exists");
    assert_eq!(state.name, "minecraft:ancient_debris");

    // 相邻方块保持 fallback
    let neighbor = chunk::block_state_at(&root_back, 7, -60, 8)
        .unwrap()
        .expect("neighbor exists");
    assert_eq!(neighbor.name, "minecraft:deepslate");
}

#[test]
fn paths_helpers() {
    use qexed_anvil::paths;
    let root = std::path::Path::new("world");

    assert_eq!(
        paths::dimension_region_dir(root, "minecraft:overworld"),
        root.join("region")
    );
    assert_eq!(
        paths::dimension_region_dir(root, "minecraft:the_nether"),
        root.join("DIM-1").join("region")
    );
    assert_eq!(
        paths::dimension_region_dir(root, "minecraft:the_end"),
        root.join("DIM1").join("region")
    );
    assert_eq!(
        paths::region_file_name(31, 32),
        "r.0.1.mca"
    );
    assert_eq!(
        paths::region_file_name(-1, -33),
        "r.-1.-2.mca"
    );
    assert_eq!(
        paths::floor_div(-7, 2),
        -4
    );
}
