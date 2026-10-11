#![cfg(feature = "vanilla-verify")]
use qexed_anvil::region::AnvilRegion;
use qexed_nbt::{Tag, from_slice};

#[test]
fn dump_chunk_root_fields() {
    let region = AnvilRegion::from_file("../../.tmp/vanilla-light/world/dimensions/minecraft/overworld/region/r.0.0.mca").unwrap();
    let Some(data) = region.read_chunk(0, 0).unwrap() else { panic!("no chunk") };
    let raw = data.decompress().unwrap();
    let (_, root) = from_slice(&raw).unwrap();
    let compound = qexed_anvil::chunk::root_compound(&root).unwrap();
    let mut keys: Vec<&String> = compound.keys().collect();
    keys.sort();
    println!("root keys: {:?}", keys);
    for k in ["Status", "isLightOn", "InhabitedTime", "LastUpdate"] {
        if let Some(t) = compound.get(k) {
            println!("{k}: {t:?}");
        }
    }
}
