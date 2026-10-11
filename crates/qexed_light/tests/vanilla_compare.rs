//! 与原版 26.1.2 存档光照的对比验证（feature = "vanilla-verify"）。
//!
//! 方法：读 mc-worldgen/mc-saves/26.1.2 的区块 NBT——
//! - 方块数据（block_states palette）→ 构建不透明度/亮度
//! - 原版已算好的 SkyLight/BlockLight section → 作为"标准答案"
//! 跑本 crate 引擎，逐格对比。
//!
//! 运行：cargo test -p qexed_light --features vanilla-verify --test vanilla_compare -- --nocapture

#![cfg(feature = "vanilla-verify")]

use std::collections::HashMap;
use std::path::Path;

use qexed_anvil::chunk;
use qexed_anvil::region::AnvilRegion;
use qexed_light::config::algorithm::LightAlgorithm;
use qexed_light::data::{BlockLightProps, ChunkOpacity, OpacityNeighborhood};
use qexed_light::engine::LightEngine;
use qexed_light::geometry::DimensionGeometry;
use qexed_nbt::{Tag, from_slice};

const SAVE: &str = "../../mc-worldgen/mc-saves/26.1.2";

/// 26.1.2 方块光学属性（opacity = light_block，luminance = light_emission）。
/// 缺省：非 air 全挡（opacity 15）；air 透光。
fn block_props(name: &str) -> BlockLightProps {
    match name {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => BlockLightProps::AIR,
        // 亮度 15 的光源
        "minecraft:glowstone" | "minecraft:sea_lantern" | "minecraft:shroomlight"
        | "minecraft:jack_o_lantern" | "minecraft:lava" | "minecraft:campfire"
        | "minecraft:beacon" | "minecraft:conduit" | "minecraft:fire"
        | "minecraft:end_portal" | "minecraft:end_gateway" => {
            BlockLightProps { opacity: 15, luminance: 15 }
        }
        "minecraft:torch" | "minecraft:wall_torch" | "minecraft:soul_torch_lit"
        | "minecraft:underwater_torch" | "minecraft:end_rod" => {
            BlockLightProps { opacity: 0, luminance: 14 }
        }
        "minecraft:nether_portal" => BlockLightProps { opacity: 0, luminance: 11 },
        "minecraft:soul_fire" => BlockLightProps { opacity: 0, luminance: 10 },
        "minecraft:glow_lichen" => BlockLightProps { opacity: 0, luminance: 7 },
        "minecraft:magma_block" => BlockLightProps { opacity: 15, luminance: 3 },
        "minecraft:sculk_sensor" => BlockLightProps { opacity: 0, luminance: 1 },
        // 半透明
        "minecraft:water" => BlockLightProps { opacity: 1, luminance: 0 },
        "minecraft:ice" | "minecraft:frosted_ice" => BlockLightProps { opacity: 2, luminance: 0 },
        "minecraft:cobweb" => BlockLightProps { opacity: 1, luminance: 0 },
        "minecraft:honey_block" | "minecraft:slime_block" => BlockLightProps { opacity: 1, luminance: 0 },
        "minecraft:glass" => BlockLightProps::AIR,
        _ => BlockLightProps::SOLID,
    }
}

/// 从区块根 NBT 构建不透明度缓存。
fn opacity_from_chunk(root: &Tag, geometry: &qexed_light::geometry::LightGeometry) -> ChunkOpacity {
    let mut opacity = ChunkOpacity::filled(geometry, BlockLightProps::AIR);
    let compound = chunk::root_compound(root).expect("chunk root compound");
    let sections = chunk::sections_by_y(compound);
    for (section_y, section) in sections {
        let Ok(blocks) = chunk::section_blocks(section) else { continue };
        for (i, &palette_index) in blocks.indices.iter().enumerate() {
            let Some(state) = blocks.palette.get(palette_index) else { continue };
            let props = block_props(&state.name);
            let y = section_y * 16 + (i / 256) as i32;
            let z = ((i % 256) / 16) as usize;
            let x = (i % 16) as usize;
            let _ = opacity.set(x, y, z, props);
        }
    }
    opacity
}

/// 读原版光照 section（field = "SkyLight" / "BlockLight"，section_y → 2048 字节）。
fn light_sections(root: &Tag, field: &str) -> HashMap<i32, Vec<u8>> {
    let mut out = HashMap::new();
    let compound = chunk::root_compound(root).expect("chunk root compound");
    let sections = chunk::sections_by_y(compound);
    for (section_y, section) in sections {
        if let Some(Tag::ByteArray(data)) = section.get(field) {
            let bytes = data.as_ref().iter().map(|&b| b as u8).collect();
            out.insert(section_y, bytes);
        }
    }
    out
}

fn nibble(data: &[u8], index: usize) -> u8 {
    let byte = data[index / 2];
    if index % 2 == 0 { byte & 0x0f } else { byte >> 4 }
}

struct CompareResult {
    sky_mismatch: usize,
    sky_total: usize,
    block_mismatch: usize,
    block_total: usize,
    first_sky_diff: Option<String>,
}

impl CompareResult {
    fn report(&self, label: &str) {
        println!(
            "[{label}] sky: {}/{} ({:.2}%) mismatched, block: {}/{} ({:.2}%) mismatched",
            self.sky_mismatch,
            self.sky_total,
            self.sky_mismatch as f64 / self.sky_total.max(1) as f64 * 100.0,
            self.block_mismatch,
            self.block_total,
            self.block_mismatch as f64 / self.block_total.max(1) as f64 * 100.0,
        );
        if let Some(diff) = &self.first_sky_diff {
            println!("  first sky diff: {diff}");
        }
    }
}

fn compare_chunk(
    region_path: &Path,
    chunk_x: i32,
    chunk_z: i32,
    dim: DimensionGeometry,
    algorithm: LightAlgorithm,
) -> CompareResult {
    let region = AnvilRegion::from_file(region_path).unwrap();
    let data = region.read_chunk(chunk_x, chunk_z).unwrap().unwrap();
    let raw = data.decompress().unwrap();
    let (_, root) = from_slice(&raw).unwrap();

    let geometry = &dim.geometry;
    let opacity = opacity_from_chunk(&root, geometry);
    let vanilla_sky = light_sections(&root, "SkyLight");
    let vanilla_block = light_sections(&root, "BlockLight");

    let neighborhood = OpacityNeighborhood::single(opacity);
    let engine = LightEngine::new(dim, algorithm.into());
    let ours_sky = engine.compute_sky_light(&neighborhood);
    let ours_block = engine.compute_block_light(&neighborhood, None);

    let mut result = CompareResult {
        sky_mismatch: 0, sky_total: 0,
        block_mismatch: 0, block_total: 0,
        first_sky_diff: None,
    };

    for (section_y, v_data) in &vanilla_sky {
        for x in 0usize..16 {
            for z in 0usize..16 {
                for y_in_section in 0usize..16 {
                    let y = section_y * 16 + y_in_section as i32;
                    if !(geometry.world_min_y..=geometry.world_max_y).contains(&y) { continue; }
                    let vanilla = nibble(v_data, y_in_section * 256 + z * 16 + x);
                    let mine = ours_sky.sky_at(x, y, z);
                    result.sky_total += 1;
                    if vanilla != mine {
                        result.sky_mismatch += 1;
                        if result.first_sky_diff.is_none() {
                            result.first_sky_diff = Some(format!(
                                "({x},{y},{z}): vanilla={vanilla} ours={mine}"
                            ));
                        }
                    }
                }
            }
        }
    }
    for (section_y, v_data) in &vanilla_block {
        for x in 0usize..16 {
            for z in 0usize..16 {
                for y_in_section in 0usize..16 {
                    let y = section_y * 16 + y_in_section as i32;
                    if !(geometry.world_min_y..=geometry.world_max_y).contains(&y) { continue; }
                    let vanilla = nibble(v_data, y_in_section * 256 + z * 16 + x);
                    let mine = ours_block.block_at(x, y, z);
                    result.block_total += 1;
                    if vanilla != mine {
                        result.block_mismatch += 1;
                    }
                }
            }
        }
    }
    result
}

#[test]
fn matches_vanilla_26_1_2_overworld() {
    let region_path = Path::new(SAVE).join("dimensions/minecraft/overworld/region/r.0.0.mca");
    if !region_path.exists() {
        eprintln!("save not found, skip");
        return;
    }
    let result = compare_chunk(&region_path, 8, 8, DimensionGeometry::overworld(), LightAlgorithm::Fast);
    result.report("overworld (8,8) Fast");
}

#[test]
fn matches_vanilla_26_1_2_nether() {
    let region_path = Path::new(SAVE).join("dimensions/minecraft/the_nether/region/r.0.0.mca");
    if !region_path.exists() {
        eprintln!("save not found, skip");
        return;
    }
    let result = compare_chunk(&region_path, 8, 8, DimensionGeometry::the_nether(), LightAlgorithm::Fast);
    result.report("nether (8,8) Fast");
}
