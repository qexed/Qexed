use qexed_nbt::Tag;
fn main() {
    for name in ["1.17.1-custom-heights.chunk", "1.17.1.chunk"] {
        let bytes = std::fs::read(format!(".tmp/anvil_fixtures/{name}")).unwrap();
        let (_, root) = qexed_nbt::from_slice_lossy(&bytes).unwrap();
        let fields = qexed_anvil::chunk::root_compound(&root).unwrap();
        let Tag::Compound(level) = fields.get("Level").unwrap() else { panic!() };
        eprintln!("{name}: DataVersion={:?} Status={:?}",
            fields.get("DataVersion"), level.get("Status"));
        if let Some(Tag::IntArray(a)) = level.get("Biomes") {
            eprintln!("  Biomes len={} =", a.len());
            eprintln!("  first 64: {:?}", &a[..64.min(a.len())]);
            let set: std::collections::BTreeSet<i32> = a.iter().copied().collect();
            eprintln!("  unique values: {:?}", set);
        }
    }
}