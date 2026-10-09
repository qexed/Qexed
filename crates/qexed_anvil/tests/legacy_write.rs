//! 旧版（1.13–1.17）格式的写入兼容：修改后保持 Level.Sections 布局与字节布局。

use std::path::PathBuf;

use qexed_anvil::chunk::{self, BlockStateRef};
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

fn load_legacy(name: &str) -> Option<Tag> {
    let dir = fixtures_dir()?;
    let path = dir.join(name);
    if !path.exists() {
        return None;
    };
    let bytes = std::fs::read(path).unwrap();
    Some(qexed_nbt::from_slice_lossy(&bytes).unwrap().1)
}

/// section 的旧版存储字段（用于对比未被修改的 section）
fn legacy_section_bytes(root: &Tag, section_y: i32) -> Option<(Vec<i64>, usize)> {
    let fields = chunk::root_compound(root).unwrap();
    let Tag::Compound(level) = fields.get("Level")? else { return None };
    let Tag::List(_, sections) = level.get("Sections")? else { return None };
    for section in sections.iter() {
        let Tag::Compound(section_map) = section else { continue };
        if chunk::root_compound(section).ok().map(|m| m.get("Y")).is_none() { continue }
        let y = match section_map.get("Y") {
            Some(Tag::Byte(y)) => i32::from(*y),
            _ => continue,
        };
        if y != section_y { continue }
        let Tag::LongArray(data) = section_map.get("BlockStates")? else { return None };
        let Tag::List(_, palette) = section_map.get("Palette")? else { return None };
        return Some((data.to_vec(), palette.len()));
    }
    None
}

#[test]
fn set_block_preserves_legacy_layout() {
    let Some(root) = load_legacy("1.17.1.chunk") else { return };

    // 修改前记录一个未触碰 section 与目标 section 的原始字节
    let untouched_before = legacy_section_bytes(&root, 3);
    let target_before = legacy_section_bytes(&root, 0);
    assert!(target_before.is_some(), "fixture has section 0");

    let target = BlockStateRef::new("minecraft:diamond_block");
    let updated = chunk::set_block_state(
        &root,
        5,
        5, // section 0 内
        5,
        &target,
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    )
    .unwrap();

    // 1) 布局保持：Level.Sections 存在，根级没有混入新版 sections
    let fields = chunk::root_compound(&updated).unwrap();
    assert!(fields.get("Level").is_some(), "Level wrapper preserved");
    assert!(fields.get("sections").is_none(), "no modern sections injected");

    // 2) 未修改 section 的存储字节不变
    let untouched_after = legacy_section_bytes(&updated, 3);
    assert_eq!(untouched_before, untouched_after);

    // 3) 读回目标方块
    let read_back = chunk::block_state_at(&updated, 5, 5, 5).unwrap().unwrap();
    assert_eq!(read_back, target);

    // 4) 修改发生在旧版字段里（Palette/BlockStates）
    let target_after = legacy_section_bytes(&updated, 0);
    let (before_data, before_len) = target_before.unwrap();
    let (after_data, after_len) = target_after.unwrap();
    // palette 增长（新方块加入）或不变（复用）；data 布局仍是旧版长整型
    assert!(after_len >= before_len);
    assert_eq!(after_data.len(), before_data.len().max(after_data.len()));
    assert!(!after_data.is_empty());
}

#[test]
fn set_block_in_missing_legacy_section_creates_legacy_section() {
    let Some(root) = load_legacy("1.17.1.chunk") else { return };

    // 选一个 fixture 里不存在的 section
    assert!(legacy_section_bytes(&root, 30).is_none());

    let bedrock = BlockStateRef::new("minecraft:bedrock");
    let updated = chunk::set_block_state(
        &root,
        0,
        480, // section 30
        0,
        &bedrock,
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    )
    .unwrap();

    // 新 section 用旧版字段创建，且只含 bedrock
    let (data, palette_len) = legacy_section_bytes(&updated, 30).expect("created");
    // palette = [fallback 填充方块, 目标方块]；palette > 1 时带 BlockStates 位打包
    assert_eq!(palette_len, 2, "palette = fallback + target");
    assert!(!data.is_empty(), "multi-entry palette carries BlockStates");

    let read_back = chunk::block_state_at(&updated, 0, 480, 0).unwrap().unwrap();
    assert_eq!(read_back, bedrock);
}

#[test]
fn legacy_roundtrip_through_set_is_stable() {
    let Some(root) = load_legacy("etho.chunk") else { return };

    // 连续两次相同修改 = 幂等
    let target = BlockStateRef::new("minecraft:gold_block");
    let args = (
        &root,
        2i32,
        70i32,
        2i32,
        &target,
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    );
    let once = chunk::set_block_state(args.0, args.1, args.2, args.3, args.4, args.5, args.6).unwrap();
    let twice = chunk::set_block_state(&once, args.1, args.2, args.3, args.4, args.5, args.6).unwrap();

    // NBT 序列化字节一致（同布局同 palette）
    // 语义等价（HashMap 字段顺序在序列化间不稳定，比较结构而非字节）
    // 加一层序列化-再解析的往返，确保写出的 NBT 能原样读回
    let bytes_once = qexed_nbt::to_vec("", &once).unwrap();
    let reparsed = qexed_nbt::from_slice(&bytes_once).unwrap().1;
    assert_eq!(reparsed, twice, "set_block_state is idempotent (semantically)");
    assert_eq!(once, twice);
}
