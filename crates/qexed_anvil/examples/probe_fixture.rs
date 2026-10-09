//! 调试工具：打印 fixture 的顶层 NBT 键（识别区块格式差异）。
use std::io::Read;

fn load(path: &str) -> qexed_nbt::Tag {
    let bytes = std::fs::read(path).unwrap();
    match qexed_nbt::from_slice(&bytes) {
        Ok((name, root)) => {
            eprintln!("{path}: raw NBT (root name '{name}')");
            root
        }
        Err(err) => {
            eprintln!("{path}: raw NBT parse failed: {err}");
            let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
            let mut raw = Vec::new();
            decoder.read_to_end(&mut raw).unwrap();
            let (name, root) = qexed_nbt::from_slice(&raw).unwrap();
            eprintln!("{path}: gzip NBT (root name '{name}')");
            root
        }
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let root = load(&path);
        if let qexed_nbt::Tag::Compound(map) = &root {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            eprintln!("  keys: {:?}", keys);
            // sections 形态
            if let Some(qexed_nbt::Tag::List(header, items)) = map.get("sections") {
                eprintln!("  sections: {} items (tag {})", items.len(), header.tag_id);
                if let Some(qexed_nbt::Tag::Compound(first)) = items.first() {
                    let mut sk: Vec<&String> = first.keys().collect();
                    sk.sort();
                    eprintln!("  first section keys: {:?}", sk);
                }
            }
            if let Some(qexed_nbt::Tag::Compound(level)) = map.get("Level") {
                let mut lk: Vec<&String> = level.keys().collect();
                lk.sort();
                eprintln!("  Level keys: {:?}", lk);
                if let Some(qexed_nbt::Tag::List(_, items)) = level.get("Sections") {
                    eprintln!("  legacy Sections: {} items", items.len());
                    let populated = items.iter().filter_map(|s| match s { qexed_nbt::Tag::Compound(c) if c.contains_key("Palette") || c.contains_key("Blocks") => Some(c), _ => None }).next();
                    if let Some(first) = populated {
                        let mut sk: Vec<&String> = first.keys().collect();
                        sk.sort();
                        eprintln!("  first legacy section keys: {:?}", sk);
                        if let Some(qexed_nbt::Tag::List(_, palette)) = first.get("Palette") {
                            eprintln!("  Palette entries: {}", palette.len());
                            if let Some(qexed_nbt::Tag::Compound(entry)) = palette.first() {
                                let mut pk: Vec<&String> = entry.keys().collect();
                                pk.sort();
                                eprintln!("  palette entry keys: {:?}", pk);
                            }
                        }
                    }
                }
            }
        } else {
            eprintln!("  root is not a compound: {:?}", root.tag_id());
        }
    }
}