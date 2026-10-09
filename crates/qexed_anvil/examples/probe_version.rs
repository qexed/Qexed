//! 查看区域文件里所有区块的 DataVersion 分布。
use qexed_anvil::region::AnvilRegion;

fn main() {
    let path = std::env::args().nth(1).expect("usage: probe_version <mca>");
    let region = AnvilRegion::from_file(&path).unwrap();
    let mut versions = std::collections::BTreeMap::new();
    let mut legacy = 0usize;
    for (lx, lz) in region.chunk_coords() {
        let Some(data) = region.read_chunk(lx, lz).unwrap() else { continue };
        let Ok(raw) = data.decompress() else { continue };
        let Ok((_, root)) = qexed_nbt::from_slice_lossy(&raw) else { continue };
        let Ok(fields) = qexed_anvil::chunk::root_compound(&root) else { continue };
        let v = qexed_anvil::chunk::data_version(fields).unwrap_or(-1);
        let is_legacy = fields.contains_key("Level");
        if is_legacy { legacy += 1; }
        *versions.entry(v).or_insert(0) += 1;
    }
    eprintln!("{path}: legacy(Level)={legacy}");
    for (v, n) in versions { eprintln!("  DataVersion {v}: {n} chunks"); }
}