//! chunk.rs 的 NBT 构造/读取往返测试（不落盘，纯内存）。

use std::collections::HashMap;
use std::sync::Arc;

use qexed_anvil::chunk::{
    self, BlockStateRef, empty_section, section_blocks, set_block_state,
};
use qexed_nbt::Tag;

fn root_with_section(section_y: i32) -> Tag {
    let section = empty_section(
        section_y,
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    );
    let sections = qexed_nbt::Tag::List(
        qexed_nbt::ListHeader {
            tag_id: qexed_nbt::tag_id::COMPOUND,
            length: 1,
        },
        Arc::from(vec![Tag::Compound(Arc::new(section))]),
    );
    let mut root = HashMap::new();
    root.insert("sections".to_string(), sections);
    Tag::Compound(Arc::new(root))
}

#[test]
fn empty_section_reads_back_as_fallback() {
    let root = root_with_section(0);
    let state = chunk::block_state_at(&root, 3, 5, 7).unwrap().unwrap();
    assert_eq!(state.name, "minecraft:stone");
    assert!(state.properties.is_empty());
}

#[test]
fn missing_section_returns_none() {
    let root = root_with_section(0);
    // y=100 在 section 6，不存在
    assert!(chunk::block_state_at(&root, 0, 100, 0).unwrap().is_none());
}

#[test]
fn set_then_read_roundtrips_state() {
    let root = root_with_section(-4);
    let diamond_ore = BlockStateRef::new("minecraft:diamond_ore")
        .with_properties(vec![("lit".to_string(), "false".to_string())]);

    let updated = set_block_state(
        &root,
        10,
        -60, // section -4 内
        15,
        &diamond_ore,
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    )
    .unwrap();

    let read_back = chunk::block_state_at(&updated, 10, -60, 15)
        .unwrap()
        .expect("state exists after set");
    assert_eq!(read_back, diamond_ore);

    // 其他位置不受影响
    let untouched = chunk::block_state_at(&updated, 0, -60, 0).unwrap().unwrap();
    assert_eq!(untouched.name, "minecraft:stone");
}

#[test]
fn set_in_missing_section_creates_it() {
    let root = root_with_section(0);
    let deep = BlockStateRef::new("minecraft:bedrock");

    let updated = set_block_state(
        &root,
        0,
        -64, // section -4 不存在 -> 创建
        0,
        &deep,
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    )
    .unwrap();

    assert_eq!(
        chunk::block_state_at(&updated, 0, -64, 0)
            .unwrap()
            .unwrap(),
        deep
    );
    // 原 section 0 仍在
    assert!(chunk::block_state_at(&updated, 0, 5, 0).unwrap().is_some());
}

#[test]
fn sections_sorted_after_creation() {
    let root = root_with_section(4);
    let updated = set_block_state(
        &root,
        0,
        -64,
        0,
        &BlockStateRef::new("minecraft:bedrock"),
        &BlockStateRef::new("minecraft:stone"),
        "minecraft:plains",
    )
    .unwrap();

    let sections = chunk::root_compound(&updated).unwrap();
    let ys: Vec<i32> = chunk::sections_by_y(sections)
        .keys()
        .copied()
        .collect();
    let mut sorted = ys.clone();
    sorted.sort();
    assert_eq!(ys.len(), 2);
    // 列表顺序（sections list 的 Y 字段）单调递增
    let Tag::List(_, items) = sections.get("sections").unwrap() else {
        panic!("sections is a list");
    };
    let mut list_ys = Vec::new();
    for item in items.iter() {
        let Tag::Compound(fields) = item else { continue };
        if let Some(Tag::Byte(y)) = fields.get("Y") {
            list_ys.push(i32::from(*y));
        }
    }
    let mut sorted_list = list_ys.clone();
    sorted_list.sort();
    assert_eq!(list_ys, sorted_list);
}

#[test]
fn palette_indices_expand_to_entry_count() {
    let root = root_with_section(0);
    let root_map = chunk::root_compound(&root).unwrap();
    let sections = chunk::sections_by_y(root_map);
    let section = sections.get(&0).unwrap();
    let blocks = section_blocks(section).unwrap();
    assert_eq!(blocks.indices.len(), 4096);
    assert!(blocks.indices.iter().all(|&index| index == 0));
}
