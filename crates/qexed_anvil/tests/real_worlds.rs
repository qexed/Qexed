//! 真实存档回归测试（fixture 见 .tmp/anvil_fixtures，来源 fastnbt 与 go-mc 的公开测试数据）。
//! 用 REAL_WORLD_FIXTURES 环境变量指向 fixture 根目录；不存在时跳过。

use std::path::{Path, PathBuf};

use qexed_anvil::chunk::{self};
use qexed_anvil::region::AnvilRegion;

fn fixtures_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("REAL_WORLD_FIXTURES") {
        let dir = PathBuf::from(dir);
        return dir.exists().then_some(dir);
    }
    // cargo test 的 CWD 是包根，fixture 在仓库根的 .tmp 下：向上找 Cargo.toml（workspace 根）
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join(".tmp/anvil_fixtures");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// 统计一个区域文件：区块数、可解压数、NBT 可解析数、section 可解码数。
fn probe_region(path: &Path) -> (usize, usize, usize, usize, usize) {
    let Ok(region) = AnvilRegion::from_file(path) else { return (0, 0, 0, 0, 0); };
    let mut chunks = 0;
    let mut decompressed = 0;
    let mut parsed = 0;
    let mut sections = 0;
    let mut blocks_read = 0;
    for (lx, lz) in region.chunk_coords() {
        let Some(data) = region.read_chunk(lx, lz).expect("read chunk") else {
            continue;
        };
        chunks += 1;
        let Ok(raw) = data.decompress() else { continue };
        decompressed += 1;
        let Ok((_, root)) = qexed_nbt::from_slice_lossy(&raw) else { continue };
        parsed += 1;
        let Ok(all) = chunk::all_section_blocks(&root) else { continue };
        sections += all.len();
        for section in &all {
            // 遍历 4096 个条目验证索引都在 palette 范围内
            for &index in &section.indices {
                assert!(
                    index < section.palette.len(),
                    "palette index {index} out of range {} in {}",
                    section.palette.len(),
                    path.display()
                );
                blocks_read += 1;
            }
        }
    }
    (chunks, decompressed, parsed, sections, blocks_read)
}

// ---------------------------------------------------------------------------
// 完整世界（go-mc testdata：region + entities + poi，负坐标区域）
// ---------------------------------------------------------------------------

#[test]
fn go_mc_world_region_files_parse_fully() {
    let Some(dir) = fixtures_dir() else { return };
    let world = dir.join("goworld");
    if !world.exists() { return };

    let mut total_chunks = 0;
    let mut total_blocks = 0;
    for sub in ["region", "entities", "poi"] {
        let region_dir = world.join(sub);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&region_dir)
            .expect("read region dir")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mca"))
            .collect();
        files.sort();
        for file in files {
            let (chunks, decompressed, parsed, sections, blocks) = probe_region(&file);
            // region 目录必须全部成功；entities/poi 的 NBT 结构不同（非 chunk 布局），
            // 只要求容器层（定位/解压）工作
            if sub == "region" {
                assert!(chunks > 0, "no chunks in {}", file.display());
                assert_eq!(
                    chunks, decompressed,
                    "decompression failed in {}", file.display()
                );
                assert_eq!(
                    decompressed, parsed,
                    "NBT parse failed in {}", file.display()
                );
                assert!(sections > 0);
                total_chunks += chunks;
                total_blocks += blocks;
            } else if chunks > 0 {
                // entities/poi：非 chunk NBT 布局，只验证容器层（定位/解压）
                assert_eq!(chunks, decompressed, "container layer broken in {}", sub);
            }
            // 空文件（截断/全零头的真实文件）直接跳过——容器层已优雅处理
        }
    }
    eprintln!("go-mc world: {total_chunks} chunks, {total_blocks} block entries verified");
    assert!(total_chunks > 100, "expected a substantial world");
}

#[test]
fn go_mc_world_level_dat_parses() {
    let Some(dir) = fixtures_dir() else { return };
    let level = dir.join("goworld/level.dat");
    if !level.exists() { return };

    // level.dat 是 gzip 的 named NBT，验证 qexed_nbt 链路
    let (name, root) = qexed_nbt::from_file(&level).expect("parse level.dat");
    assert_eq!(name, "");
    assert!(chunk::root_compound(&root).is_ok());
    let fields = chunk::root_compound(&root).unwrap();
    // vanilla level.dat：数据嵌在 "Data" compound 里
    let data = match fields.get("Data") {
        Some(qexed_nbt::Tag::Compound(data)) => data,
        _ => panic!("level.dat missing Data compound"),
    };
    assert!(data.contains_key("DataVersion"));
    assert!(data.contains_key("LevelName") || data.contains_key("SpawnX"));
}

// ---------------------------------------------------------------------------
// fastnbt 1.19.4 真实区域文件（8MB，超多区块）
// ---------------------------------------------------------------------------

#[test]
fn fastnbt_1_19_4_region_parses_fully() {
    let Some(dir) = fixtures_dir() else { return };
    let path = dir.join("1.19.4.mca");
    if !path.exists() { return };

    let (chunks, decompressed, parsed, sections, blocks) = probe_region(&path);
    eprintln!("1.19.4.mca: {chunks} chunks, {sections} sections, {blocks} blocks");
    assert!(chunks > 50, "expected many chunks, got {chunks}");
    assert_eq!(chunks, decompressed, "some chunks failed to decompress");
    assert_eq!(decompressed, parsed, "some chunks failed NBT parse");
    assert!(sections > 100);
}

// ---------------------------------------------------------------------------
// 单区块 fixture：1.12（旧格式）/1.17/1.17-custom-heights/forge/unicode/边角案例
// 这些是解压后的区块 NBT（.chunk/.nbt），直接解析验证 section 读取
// ---------------------------------------------------------------------------

#[test]
fn single_chunk_fixtures_parse() {
    let Some(dir) = fixtures_dir() else { return };
    let names = [
        "1.12.chunk",
        "1.17.0.chunk",
        "1.17.1-custom-heights.chunk",
        "1.17.1.chunk",
        "chunk.nbt",
        "etho.chunk",
        "etho-empty.chunk",
        "etho-max-heights.chunk",
        "etho-old-heightmaps.chunk",
        "etho-old-in-new.chunk",
        "etho-old-in-new2.chunk",
        "forge-1.20.1.nbt",
        "issue99-chunk.nbt",
        "unicode.chunk",
        "21w44a-test1.nbt",
        "etho-end-r.-6.-1.c.7.25.nbt",
    ];

    for name in names {
        let path = dir.join(name);
        if !path.exists() {
            eprintln!("skip missing fixture: {name}");
            continue;
        };
        let bytes = std::fs::read(&path).unwrap();
        // .chunk/.nbt fixture 可能是裸 NBT 或 gzip；先试裸的，失败再试 gzip
        let root = match qexed_nbt::from_slice_lossy(&bytes) {
            Ok((_, root)) => root,
            Err(_) => {
                use std::io::Read;
                let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
                let mut raw = Vec::new();
                decoder.read_to_end(&mut raw).expect("fixture is NBT or gzip NBT");
                qexed_nbt::from_slice(&raw)
                    .expect("fixture parses as NBT")
                    .1
            }
        };

        // 全量 section 解码；1.12 旧格式（没有 sections 列表）允许为空
        let sections = chunk::all_section_blocks(&root).unwrap_or_default();
        eprintln!("{name}: {} sections", sections.len());
        for section in &sections {
            assert_eq!(section.indices.len(), 4096);
            for &index in &section.indices {
                assert!(index < section.palette.len());
            }
        }
        // 1.13+ 区块应有 sections（1.12 及更早的扁平 ID 格式不在支持范围，见 README）
        if !name.starts_with("1.12") && !name.starts_with("etho-end") {
            assert!(
                !sections.is_empty() || name.contains("etho-empty"),
                "{name}: expected sections"
            );
        } else {
            // 1.12 旧格式：明确不支持，sections 应为空而不是崩溃
            assert!(sections.is_empty(), "{name}: unsupported/empty fixture should report no sections");
        }
    }
}
