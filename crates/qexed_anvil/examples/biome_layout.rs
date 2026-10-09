//! 实证：旧版 Biomes 数组长度与取值分布（判断 4 层布局是否成立）。
fn main() {
    for name in ["1.17.0.chunk", "1.17.1.chunk", "1.17.1-custom-heights.chunk", "etho.chunk", "unicode.chunk"] {
        let Ok(bytes) = std::fs::read(format!(".tmp/anvil_fixtures/{name}")) else { continue };
        let Ok((_, root)) = qexed_nbt::from_slice_lossy(&bytes) else { continue };
        let Ok(fields) = qexed_anvil::chunk::root_compound(&root) else { continue };
        let Some(qexed_nbt::Tag::Compound(level)) = fields.get("Level") else { continue };
        let (biomes, sections) = (level.get("Biomes"), level.get("Sections"));
        let blen = match biomes {
            Some(qexed_nbt::Tag::IntArray(a)) => format!("int[{}]", a.len()),
            Some(qexed_nbt::Tag::ByteArray(a)) => format!("byte[{}]", a.len()),
            _ => "none".to_string(),
        };
        let sec_ys: Vec<i32> = match sections {
            Some(qexed_nbt::Tag::List(_, items)) => items.iter().filter_map(|s| match s {
                qexed_nbt::Tag::Compound(f) => f.get("Y").and_then(|y| match y {
                    qexed_nbt::Tag::Byte(v) => Some(i32::from(*v)), _ => None }),
                _ => None }).collect(),
            _ => vec![],
        };
        let layers = match biomes {
            Some(qexed_nbt::Tag::IntArray(a)) if a.len() == 1024 => (0..4).map(|l| {
                let set: std::collections::BTreeSet<i32> = a[l*256..(l+1)*256].iter().copied().collect();
                set.len()
            }).collect::<Vec<_>>(),
            _ => vec![],
        };
        eprintln!("{name}: Biomes={blen} sections={sec_ys:?} per-layer-unique={layers:?}");
    }
}