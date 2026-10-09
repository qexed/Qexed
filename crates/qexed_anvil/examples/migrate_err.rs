fn main() {
    let path = std::env::args().nth(1).expect("usage: <mca>");
    let region = qexed_anvil::region::AnvilRegion::from_file(&path).unwrap();
    let mut ok = 0;
    for (lx, lz) in region.chunk_coords() {
        let data = region.read_chunk(lx, lz).unwrap().unwrap();
        let raw = data.decompress().unwrap();
        let (_, root) = qexed_nbt::from_slice_lossy(&raw).unwrap();
        match qexed_anvil::migrate::migrate_chunk(&root, &qexed_anvil::migrate::MigrateOptions::default()) {
            Ok(_) => ok += 1,
            Err(e) => eprintln!("chunk ({lx},{lz}): {e}"),
        }
    }
    eprintln!("ok={ok}");
}
