use qexed_nbt::Tag;
fn main() {
    let bytes = std::fs::read(".tmp/anvil_fixtures/1.12.chunk").unwrap();
    let (_, root) = qexed_nbt::from_slice_lossy(&bytes).unwrap();
    let fields = qexed_anvil::chunk::root_compound(&root).unwrap();
    let Tag::Compound(level) = fields.get("Level").unwrap() else { panic!() };
    let Tag::List(_, sections) = level.get("Sections").unwrap() else { panic!() };
    for s in sections.iter() {
        let Tag::Compound(sec) = s else { continue };
        let y = sec.get("Y");
        let blocks_len = match sec.get("Blocks") { Some(Tag::ByteArray(b)) => b.len(), _ => 0 };
        let data_len = match sec.get("Data") { Some(Tag::ByteArray(b)) => b.len(), _ => 0 };
        eprintln!("section Y={y:?} Blocks={blocks_len} Data={data_len}");
        // 前几个非零方块 ID
        if let Some(Tag::ByteArray(b)) = sec.get("Blocks") {
            let distinct: std::collections::BTreeMap<i8, usize> = b.iter().fold(Default::default(), |mut m: std::collections::BTreeMap<i8,usize>, v| { *m.entry(*v).or_default() += 1; m });
            eprintln!("  ids: {:?}", distinct);
        }
    }
    match level.get("Entities") {
        Some(Tag::List(h, items)) => eprintln!("Entities: {} items (tag {})", items.len(), h.tag_id),
        other => eprintln!("Entities: {other:?}"),
    }
    if let Some(Tag::List(_, items)) = level.get("Entities") {
        for e in items.iter().take(3) {
            if let Tag::Compound(ent) = e {
                eprintln!("  entity keys: {:?}", ent.keys().collect::<Vec<_>>());
            }
        }
    }
}