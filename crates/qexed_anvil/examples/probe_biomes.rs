//! 查看 1.17 区块的 Biomes 字段形态。
fn main() {
    let bytes = std::fs::read(".tmp/anvil_fixtures/1.17.1.chunk").unwrap();
    let (_, root) = qexed_nbt::from_slice_lossy(&bytes).unwrap();
    let fields = qexed_anvil::chunk::root_compound(&root).unwrap();
    let Tag::Compound(level) = fields.get("Level").unwrap() else { panic!() };
    match level.get("Biomes") {
        Some(qexed_nbt::Tag::ByteArray(b)) => {
            eprintln!("Biomes: byte[{}] first 16: {:?}", b.len(), &b[..16.min(b.len())]);
        }
        Some(qexed_nbt::Tag::IntArray(a)) => {
            eprintln!("Biomes: int[{}] first 16: {:?}", a.len(), &a[..16.min(a.len())]);
        }
        other => eprintln!("Biomes: {other:?}"),
    }
    // 也看看 Status 与 yPos
    eprintln!("Status: {:?}", level.get("Status"));
    eprintln!("xPos/zPos: {:?}/{:?}", level.get("xPos"), level.get("zPos"));
    eprintln!("root DataVersion: {:?}", fields.get("DataVersion"));
    eprintln!("Level keys count: {}", level.len());
    for (k, v) in level.iter() {
        let kind = match v {
            qexed_nbt::Tag::Compound(_) => "compound",
            qexed_nbt::Tag::List(_, _) => "list",
            qexed_nbt::Tag::ByteArray(a) => &format!("byte[{}]", a.len()),
            qexed_nbt::Tag::LongArray(a) => &format!("long[{}]", a.len()),
            _ => "scalar",
        };
        eprintln!("  {k}: {kind}");
    }
}

use qexed_nbt::Tag;