fn main() {
    let path = std::env::args().nth(1).expect("usage: probe <mca>");
    let region = qexed_anvil::region::AnvilRegion::from_file(&path).unwrap();
    let (lx, lz) = region.chunk_coords().first().copied().unwrap_or((0, 0));
    let data = region.read_chunk(lx, lz).unwrap().unwrap();
    let raw = data.decompress().unwrap();
    let (_, root) = qexed_nbt::from_slice_lossy(&raw).unwrap();
    let fields = qexed_anvil::chunk::root_compound(&root).unwrap();
    eprintln!("compression={} root keys: {:?}", data.compression, fields.keys().collect::<Vec<_>>());
    if let Some(qexed_nbt::Tag::Compound(level)) = fields.get("Level") {
        eprintln!("Level keys: {:?}", level.keys().collect::<Vec<_>>());
        if let Some(qexed_nbt::Tag::List(_, sections)) = level.get("Sections") {
            eprintln!("{} sections", sections.len());
            if let Some(qexed_nbt::Tag::Compound(s)) = sections.iter().next() {
                eprintln!("first section keys: {:?}", s.keys().collect::<Vec<_>>());
            }
        }
    }
}
