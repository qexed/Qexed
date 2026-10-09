//! 验证：真实 1.17 区块的 BlockStates 解包后重新打包，字节级一致。

fn main() {
    let path = std::env::args().nth(1).expect("usage: legacy_roundtrip <1.17 chunk>");
    let bytes = std::fs::read(&path).unwrap();
    let (_, root) = qexed_nbt::from_slice_lossy(&bytes).unwrap();
    let root_map = qexed_anvil::chunk::root_compound(&root).unwrap();
    let level = match root_map.get("Level") {
        Some(qexed_nbt::Tag::Compound(level)) => level.as_ref(),
        _ => panic!("not a legacy chunk"),
    };
    let qexed_nbt::Tag::List(_, sections) = level.get("Sections").unwrap() else { panic!() };

    let mut checked = 0;
    for section in sections.iter() {
        let qexed_nbt::Tag::Compound(fields) = section else { continue };
        let Some(qexed_nbt::Tag::LongArray(original)) = fields.get("BlockStates") else { continue };
        let Some(qexed_nbt::Tag::List(_, palette)) = fields.get("Palette") else { continue };
        if palette.len() <= 1 { continue };

        let blocks = qexed_anvil::chunk::section_blocks(fields).unwrap();
        // 用同样的 scheme 重新打包
        let repacked = qexed_anvil::palette::pack_values(
            &blocks.indices,
            qexed_anvil::palette::storage_bits(
                qexed_anvil::palette::PaletteKind::Block,
                blocks.palette.len(),
            ),
        )
        .unwrap();
        let repacked_i64: Vec<i64> = repacked.into_iter().map(|v| v as i64).collect();
        assert_eq!(original.as_ref(), &repacked_i64[..], "section Y={} mismatch",
            qexed_anvil::chunk::root_compound(section).unwrap().get("Y").is_some());
        checked += 1;
    }
    eprintln!("OK: {checked} sections round-trip byte-identical");
}