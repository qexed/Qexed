//! 端到端：真实旧版区块 -> 修改 -> 写入 .mca -> 读回，布局与数据都保持旧版。

use std::path::PathBuf;

use qexed_anvil::chunk::{self, BlockStateRef};
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

#[test]
fn legacy_chunk_survives_region_file_roundtrip() {
    let Some(dir) = fixtures_dir() else { return };
    let src = dir.join("1.17.1.chunk");
    if !src.exists() { return };
    let bytes = std::fs::read(&src).unwrap();
    let root = qexed_nbt::from_slice_lossy(&bytes).unwrap().1;

    // 修改两个方块（已有 section + 新建 section 各一）
    let gold = BlockStateRef::new("minecraft:gold_block");
    let updated = chunk::set_block_state(
        &root, 8, 40, 8, &gold,
        &BlockStateRef::new("minecraft:stone"), "minecraft:plains",
    ).unwrap();
    let bedrock = BlockStateRef::new("minecraft:bedrock");
    let updated = chunk::set_block_state(
        &updated, 3, 500, 3, &bedrock,   // section 31：原区块没有
        &BlockStateRef::new("minecraft:stone"), "minecraft:plains",
    ).unwrap();

    // 序列化 -> 压缩 -> 写入区域文件
    let out_nbt = qexed_nbt::to_vec("", &updated).unwrap();
    let chunk_data = ChunkData::zlib(&out_nbt).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let mca = tmp.path().join("r.0.0.mca");
    let mut region = AnvilRegion::new(&mca);
    region.write_chunk(0, 0, chunk_data).unwrap();
    region.save().unwrap();

    // 读回 -> 解析 -> 验证
    let region = AnvilRegion::from_file(&mca).unwrap();
    let data = region.read_chunk(0, 0).unwrap().expect("chunk exists");
    let raw = data.decompress().unwrap();
    let back = qexed_nbt::from_slice(&raw).unwrap().1;

    // 布局仍是旧版
    let fields = chunk::root_compound(&back).unwrap();
    assert!(fields.get("Level").is_some(), "legacy layout preserved through region file");
    assert!(fields.get("sections").is_none());

    // 两处方块都读得回来
    assert_eq!(chunk::block_state_at(&back, 8, 40, 8).unwrap().unwrap(), gold);
    assert_eq!(chunk::block_state_at(&back, 3, 500, 3).unwrap().unwrap(), bedrock);

    // 语义等价（幂等）
    assert_eq!(back, updated);
}
