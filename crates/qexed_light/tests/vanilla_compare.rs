//! 与原版 26.1.2 服务端光照的逐格对比验证（feature = "vanilla-verify"）。
//!
//! 数据来源：.tmp/vanilla-light/world —— 用官方 26.1.2 server.jar 加载
//! mc-worldgen 存档，RCON forceload 144 个区块（-6..5 × -6..5）触发 vanilla
//! 光照引擎计算，save-all 落盘后的真实服务端输出。
//!
//! 关键序列化事实：vanilla 保存时省略"整 section 等于维度默认值"的
//! SkyLight 数组（主世界默认 15：露天 section 不写盘）。因此恢复标准答案时，
//! 一个 chunk 内最高"天光地平线"（最高的 opacity>0 方块）以上的 section
//! 缺数组 = 15，其余缺数组 = 0。
//!
//! 运行：cargo test -p qexed_light --features vanilla-verify --test vanilla_compare -- --nocapture

#![cfg(feature = "vanilla-verify")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use qexed_anvil::chunk;
use qexed_anvil::region::AnvilRegion;
use qexed_light::data::{
    BlockLightProps, ChunkLightData, ChunkOpacity, LightNeighborhood, OpacityNeighborhood,
};
use qexed_light::engine::LightEngine;
use qexed_light::geometry::DimensionGeometry;
use qexed_nbt::{Tag, from_slice};

const WORLD: &str = "../../.tmp/vanilla-light/world/dimensions/minecraft/overworld/region";

/// 26.1.2 方块光学属性（opacity = light_block，luminance = light_emission）。
/// 表覆盖 mc-worldgen 存档实排的全部 90 种方块；缺省 = 全方块（挡光 15）。
fn block_props(name: &str) -> BlockLightProps {
    match name {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => BlockLightProps::AIR,

        // 亮度 15
        "minecraft:lava" => BlockLightProps {
            opacity: 15,
            luminance: 15,
        },
        // 亮度 14
        "minecraft:wall_torch" | "minecraft:torch" => BlockLightProps {
            opacity: 0,
            luminance: 14,
        },
        // 荧石地衣 7
        "minecraft:glow_lichen" => BlockLightProps {
            opacity: 0,
            luminance: 7,
        },
        // 紫水晶：簇 5 / 大芽 4 / 中芽 2 / 小芽 1（26.1.2 实测 medium=2，非 3）
        "minecraft:amethyst_cluster" => BlockLightProps {
            opacity: 0,
            luminance: 5,
        },
        "minecraft:large_amethyst_bud" => BlockLightProps {
            opacity: 0,
            luminance: 4,
        },
        "minecraft:medium_amethyst_bud" => BlockLightProps {
            opacity: 0,
            luminance: 2,
        },
        "minecraft:small_amethyst_bud" => BlockLightProps {
            opacity: 0,
            luminance: 1,
        },
        // 岩浆块 15/3；蘑菇 1
        "minecraft:magma_block" => BlockLightProps {
            opacity: 15,
            luminance: 3,
        },
        "minecraft:brown_mushroom" => BlockLightProps {
            opacity: 0,
            luminance: 1,
        },
        // 红蘑菇不发光（26.1.2 实测 0）
        "minecraft:red_mushroom" => BlockLightProps {
            opacity: 0,
            luminance: 0,
        },
        // 点亮状态的红石矿石：亮度 9（未点亮的按 SOLID 默认）
        "minecraft:redstone_ore[lit=true]" => BlockLightProps {
            opacity: 15,
            luminance: 9,
        },

        // 透光衰减 1：水系（含水生植物，水格 filter 1）、树叶、蜘蛛网
        "minecraft:water"
        | "minecraft:bubble_column"
        | "minecraft:seagrass"
        | "minecraft:tall_seagrass"
        | "minecraft:kelp"
        | "minecraft:kelp_plant" => BlockLightProps {
            opacity: 1,
            luminance: 0,
        },
        "minecraft:oak_leaves" | "minecraft:birch_leaves" | "minecraft:jungle_leaves" => {
            BlockLightProps {
                opacity: 1,
                luminance: 0,
            }
        }
        "minecraft:cobweb" => BlockLightProps {
            opacity: 1,
            luminance: 0,
        },
        // 杜鹃/开花杜鹃（类树叶 filter 1）
        "minecraft:azalea" | "minecraft:flowering_azalea" => BlockLightProps {
            opacity: 1,
            luminance: 0,
        },

        // 萤火草丛：26.1.2 实测亮度 2
        "minecraft:firefly_bush" => BlockLightProps {
            opacity: 0,
            luminance: 2,
        },
        // 非完整方块/植物：不挡光、不发光
        "minecraft:bamboo"
        | "minecraft:big_dripleaf"
        | "minecraft:big_dripleaf_stem"
        | "minecraft:small_dripleaf"
        | "minecraft:bush"
        | "minecraft:fern"
        | "minecraft:short_grass"
        | "minecraft:tall_grass"
        | "minecraft:leaf_litter"
        | "minecraft:lilac"
        | "minecraft:peony"
        | "minecraft:dandelion"
        | "minecraft:moss_carpet"
        | "minecraft:sugar_cane"
        | "minecraft:vine"
        | "minecraft:oak_fence"
        | "minecraft:rail"
        | "minecraft:iron_chain"
        | "minecraft:cocoa"
        | "minecraft:chest"
        | "minecraft:spawner" => BlockLightProps::AIR,

        // 其余全部是实心方块（石头系、泥土系、矿石、原木、西瓜、苔块、蜂巢等）
        _ => BlockLightProps::SOLID,
    }
}

struct RegionSet {
    regions: HashMap<(i32, i32), AnvilRegion>,
}

impl RegionSet {
    fn open(dir: &Path) -> Self {
        let mut regions = HashMap::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                let path = dir.join(format!("r.{dx}.{dz}.mca"));
                if path.exists() {
                    regions.insert((dx, dz), AnvilRegion::from_file(&path).unwrap());
                }
            }
        }
        Self { regions }
    }

    fn read_root(&self, chunk_x: i32, chunk_z: i32) -> Option<Tag> {
        let region = self.regions.get(&(chunk_x >> 5, chunk_z >> 5))?;
        let data = region.read_chunk(chunk_x & 31, chunk_z & 31).unwrap()?;
        let raw = data.decompress().unwrap();
        let (_, root) = from_slice(&raw).unwrap();
        Some(root)
    }
}

/// 从区块 NBT 构建不透明度缓存。
fn opacity_from_chunk(root: &Tag, geometry: &qexed_light::geometry::LightGeometry) -> ChunkOpacity {
    let mut opacity = ChunkOpacity::filled(geometry, BlockLightProps::AIR);
    let compound = chunk::root_compound(root).expect("chunk root compound");
    for (section_y, section) in chunk::sections_by_y(compound) {
        let Ok(blocks) = chunk::section_blocks(section) else {
            continue;
        };
        for (i, &palette_index) in blocks.indices.iter().enumerate() {
            let Some(state) = blocks.palette.get(palette_index) else {
                continue;
            };
            // 点亮状态的红石矿石亮度 9；其余方块按名称匹配
            let lookup = if state.name == "minecraft:redstone_ore"
                && state.properties.iter().any(|(k, v)| k == "lit" && v == "true")
            {
                "minecraft:redstone_ore[lit=true]"
            } else {
                state.name.as_str()
            };
            let props = block_props(lookup);
            let y = section_y * 16 + (i / 256) as i32;
            let z = ((i % 256) / 16) as usize;
            let x = (i % 16) as usize;
            let _ = opacity.set(x, y, z, props);
        }
    }
    opacity
}

/// 每 (x,z) 列最高的 opacity>0 方块 Y（无 → world_min_y-1）。
fn sky_horizon(
    opacity: &ChunkOpacity,
    geometry: &qexed_light::geometry::LightGeometry,
) -> [[i32; 16]; 16] {
    let mut out = [[geometry.world_min_y - 1; 16]; 16];
    for x in 0..16usize {
        for z in 0..16usize {
            for y in (geometry.world_min_y..=geometry.world_max_y).rev() {
                if opacity.opacity_at(x, y, z).unwrap_or(0) > 0 {
                    out[x][z] = y;
                    break;
                }
            }
        }
    }
    out
}

/// 读原版 SkyLight，并把省略的 section 恢复为"地平线以上 15 / 以下 0"。
fn expand_vanilla_sky(
    root: &Tag,
    horizon: &[[i32; 16]; 16],
    geometry: &qexed_light::geometry::LightGeometry,
) -> Vec<u8> {
    let mut out = vec![0_u8; geometry.chunk_light_len];
    let compound = chunk::root_compound(root).expect("chunk root compound");
    let mut present: HashMap<i32, ()> = HashMap::new();
    for (section_y, section) in chunk::sections_by_y(compound) {
        let Some(Tag::ByteArray(data)) = section.get("SkyLight") else {
            continue;
        };
        present.insert(section_y, ());
        let data: Vec<u8> = data.as_ref().iter().map(|&b| b as u8).collect();
        for y_in_section in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let index = y_in_section * 256 + z * 16 + x;
                    let byte = data[index / 2];
                    let level = if index % 2 == 0 {
                        byte & 0x0f
                    } else {
                        byte >> 4
                    };
                    let y = section_y * 16 + y_in_section as i32;
                    if (geometry.world_min_y..=geometry.world_max_y).contains(&y) {
                        out[geometry.block_index(x, y, z)] = level;
                    }
                }
            }
        }
    }
    // 恢复省略 section：地平线以上（含地平线+1 起的整个天空域）= 15
    for section_y in geometry.min_light_section_y..=geometry.max_light_section_y {
        if present.contains_key(&section_y) {
            continue;
        }
        // 该 section 是否整体在所有列的地平线之上
        let base = section_y * 16;
        let top = base + 15;
        let all_above = (0..16).all(|x| (0..16).all(|z| horizon[x as usize][z as usize] < base));
        let all_below = (0..16).all(|x| (0..16).all(|z| horizon[x as usize][z as usize] >= top));
        if all_above {
            for y in base..=top {
                if (geometry.world_min_y..=geometry.world_max_y).contains(&y) {
                    for z in 0..16usize {
                        for x in 0..16usize {
                            out[geometry.block_index(x, y, z)] = 15;
                        }
                    }
                }
            }
        } else if !all_below {
            // 混合 section 缺数组：保守记为 0（地平线在其内部，值应由数组给出，
            // 缺失说明整 section 是默认值——此处按 0 处理并留给差异统计暴露）
        }
    }
    out
}

/// 读原版 BlockLight（无默认值省略问题，缺 = 0）。
fn expand_vanilla_block(root: &Tag, geometry: &qexed_light::geometry::LightGeometry) -> Vec<u8> {
    let mut out = vec![0_u8; geometry.chunk_light_len];
    let compound = chunk::root_compound(root).expect("chunk root compound");
    for (section_y, section) in chunk::sections_by_y(compound) {
        let Some(Tag::ByteArray(data)) = section.get("BlockLight") else {
            continue;
        };
        let data: Vec<u8> = data.as_ref().iter().map(|&b| b as u8).collect();
        for y_in_section in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let index = y_in_section * 256 + z * 16 + x;
                    let byte = data[index / 2];
                    let level = if index % 2 == 0 {
                        byte & 0x0f
                    } else {
                        byte >> 4
                    };
                    let y = section_y * 16 + y_in_section as i32;
                    if (geometry.world_min_y..=geometry.world_max_y).contains(&y) {
                        out[geometry.block_index(x, y, z)] = level;
                    }
                }
            }
        }
    }
    out
}

struct ChunkInputs {
    opacity: ChunkOpacity,
    vanilla_sky: Vec<u8>,
    vanilla_block: Vec<u8>,
    /// 差异诊断用：palette 名字网格（按 block_index）
    names: Vec<String>,
}

fn block_display<'a>(
    names: &'a [String],
    geometry: &qexed_light::geometry::LightGeometry,
    x: usize,
    y: i32,
    z: usize,
) -> &'a str {
    if x >= 16 || z >= 16 || !(geometry.world_min_y..=geometry.world_max_y).contains(&y) {
        return "?";
    }
    &names[geometry.block_index(x, y, z)]
}

fn chunk_status(root: &Tag) -> Option<String> {
    chunk::root_compound(root)
        .ok()?
        .get("Status")
        .and_then(|t| match t {
            Tag::String(s) => Some(s.to_string()),
            _ => None,
        })
}

fn load_chunk_inputs(
    set: &RegionSet,
    cx: i32,
    cz: i32,
    geometry: &qexed_light::geometry::LightGeometry,
) -> Option<ChunkInputs> {
    let root = set.read_root(cx, cz)?;
    if chunk_status(&root).as_deref() != Some("minecraft:full") {
        return None;
    }
    let opacity = opacity_from_chunk(&root, geometry);
    let horizon = sky_horizon(&opacity, geometry);

    // 名字网格（诊断用）
    let mut names: Vec<String> = vec!["?".to_string(); geometry.chunk_light_len];
    {
        let compound = chunk::root_compound(&root).expect("chunk root compound");
        for (section_y, section) in chunk::sections_by_y(compound) {
            let Ok(blocks) = chunk::section_blocks(section) else {
                continue;
            };
            for (i, &palette_index) in blocks.indices.iter().enumerate() {
                if let Some(state) = blocks.palette.get(palette_index) {
                    let y = section_y * 16 + (i / 256) as i32;
                    let z = ((i % 256) / 16) as usize;
                    let x = (i % 16) as usize;
                    if (geometry.world_min_y..=geometry.world_max_y).contains(&y) {
                        // 点亮的红石矿带状态后缀，供 block_props 按状态匹配
                        let display = if state.name == "minecraft:redstone_ore"
                            && state.properties.iter().any(|(k, v)| k == "lit" && v == "true")
                        {
                            "minecraft:redstone_ore[lit=true]".to_string()
                        } else {
                            state.name.to_string()
                        };
                        names[geometry.block_index(x, y, z)] = display;
                    }
                }
            }
        }
    }

    Some(ChunkInputs {
        opacity,
        vanilla_sky: expand_vanilla_sky(&root, &horizon, geometry),
        vanilla_block: expand_vanilla_block(&root, geometry),
        names,
    })
}

struct CompareResult {
    sky_mismatch: usize,
    sky_total: usize,
    block_mismatch: usize,
    block_total: usize,
    /// ours = vanilla + 1（偏亮 1 级）的格子数
    sky_plus1: usize,
    /// 其余差异
    sky_other: usize,
    sky_samples: Vec<String>,
    block_samples: Vec<String>,
}

fn compare_center(set: &RegionSet, cx: i32, cz: i32, dim: DimensionGeometry) -> CompareResult {
    let geometry = &dim.geometry;

    let center_in = load_chunk_inputs(&set, cx, cz, geometry).expect("center chunk must be full");
    let mut opacity_nb = OpacityNeighborhood::single(center_in.opacity);
    let mut light_nb = LightNeighborhood::single(ChunkLightData::empty(geometry));
    {
        let mut center_light = ChunkLightData::empty(geometry);
        center_light
            .sky_light_mut()
            .copy_from_slice(&center_in.vanilla_sky);
        center_light
            .block_light_mut()
            .copy_from_slice(&center_in.vanilla_block);
        *light_nb.center_mut().unwrap() = center_light;
    }
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            if let Some(input) = load_chunk_inputs(&set, cx + dx, cz + dz, geometry) {
                opacity_nb.set(dx, dz, input.opacity);
                let mut data = ChunkLightData::empty(geometry);
                data.sky_light_mut().copy_from_slice(&input.vanilla_sky);
                data.block_light_mut().copy_from_slice(&input.vanilla_block);
                light_nb.set(dx, dz, data);
            }
        }
    }

    let engine = LightEngine::new(dim, qexed_light::engine::SkyLightAlgorithm::Fast);
    let ours_sky = engine.compute_sky_light_with_neighbors(&opacity_nb, &light_nb);
    let ours_block = engine.compute_block_light(&opacity_nb, Some(&light_nb));

    let mut result = CompareResult {
        sky_mismatch: 0,
        sky_total: 0,
        block_mismatch: 0,
        block_total: 0,
        sky_plus1: 0,
        sky_other: 0,
        sky_samples: Vec::new(),
        block_samples: Vec::new(),
    };
    for x in 0..16usize {
        for z in 0..16usize {
            for y in geometry.world_min_y..=geometry.world_max_y {
                let index = geometry.block_index(x, y, z);
                let v = center_in.vanilla_sky[index];
                let o = ours_sky.sky_light()[index];
                result.sky_total += 1;
                if v != o && result.sky_samples.len() < 64 {
                    let here = block_display(&center_in.names, geometry, x, y, z);
                    let above = block_display(&center_in.names, geometry, x, y + 1, z);
                    result.sky_samples.push(format!(
                        "({x},{y},{z}): v={v} o={o} here={here} above={above}"
                    ));
                }
                if v != o {
                    result.sky_mismatch += 1;
                    if o as i32 == v as i32 + 1 {
                        result.sky_plus1 += 1;
                    } else {
                        result.sky_other += 1;
                    }
                }
                let v = center_in.vanilla_block[index];
                let o = ours_block.block_light()[index];
                result.block_total += 1;
                if v != o {
                    result.block_mismatch += 1;
                    if result.block_samples.len() < 400 {
                        let here = block_display(&center_in.names, geometry, x, y, z);
                        result
                            .block_samples
                            .push(format!("({x},{y},{z}): v={v} o={o} here={here}"));
                    }
                }
            }
        }
    }
    result
}

fn report(label: &str, r: &CompareResult) {
    println!(
        "[{label}] sky: {}/{} ({:.3}%) | block: {}/{} ({:.3}%)",
        r.sky_mismatch,
        r.sky_total,
        r.sky_mismatch as f64 / r.sky_total.max(1) as f64 * 100.0,
        r.block_mismatch,
        r.block_total,
        r.block_mismatch as f64 / r.block_total.max(1) as f64 * 100.0,
    );
    for s in &r.sky_samples {
        println!("  sky {s}");
    }
    for s in &r.block_samples {
        println!("  block {s}");
    }
}

#[test]
fn matches_vanilla_overworld_samples() {
    let dir = PathBuf::from(WORLD);
    if !dir.exists() {
        eprintln!("vanilla light world not found at {dir:?}; run the forceload pipeline first");
        return;
    }
    let dim = DimensionGeometry::overworld();
    let set = RegionSet::open(&dir);
    for &(cx, cz) in &[(0, 0), (2, 2), (4, 4)] {
        let r = compare_center(&set, cx, cz, dim);
        report(&format!("chunk ({cx},{cz})"), &r);
    }
}

/// 全量普查：强制加载区（-6..5 × -6..5）全部区块 vs vanilla。
/// 分组统计：8 邻居齐全的 chunk（引擎语义完备）vs 处于加载边界缺邻居的 chunk。
#[test]
fn matches_vanilla_overworld_full_sweep() {
    let dir = PathBuf::from(WORLD);
    if !dir.exists() {
        eprintln!("vanilla light world not found at {dir:?}; run the forceload pipeline first");
        return;
    }
    let dim = DimensionGeometry::overworld();
    let set = RegionSet::open(&dir);

    let mut full_ring = CompareResult {
        sky_mismatch: 0,
        sky_total: 0,
        block_mismatch: 0,
        block_total: 0,
        sky_plus1: 0,
        sky_other: 0,
        sky_samples: Vec::new(),
        block_samples: Vec::new(),
    };
    let mut edge = CompareResult {
        sky_mismatch: 0,
        sky_total: 0,
        block_mismatch: 0,
        block_total: 0,
        sky_plus1: 0,
        sky_other: 0,
        sky_samples: Vec::new(),
        block_samples: Vec::new(),
    };
    let mut chunks_full = 0usize;
    let mut chunks_edge = 0usize;
    for cx in -6..=5 {
        for cz in -6..=5 {
            let Some(_) = load_chunk_inputs(&set, cx, cz, &dim.geometry) else {
                continue;
            };
            let has_all = (-1..=1).all(|dx| {
                (-1..=1).all(|dz| {
                    (dx == 0 && dz == 0)
                        || load_chunk_inputs(&set, cx + dx, cz + dz, &dim.geometry).is_some()
                })
            });
            let r = compare_center(&set, cx, cz, dim);
            let (target, chunk_count) = if has_all {
                chunks_full += 1;
                (&mut full_ring, &mut chunks_full)
            } else {
                chunks_edge += 1;
                (&mut edge, &mut chunks_edge)
            };
            let _ = chunk_count;
            target.sky_mismatch += r.sky_mismatch;
            target.sky_total += r.sky_total;
            target.block_mismatch += r.block_mismatch;
            target.block_total += r.block_total;
            target.sky_plus1 += r.sky_plus1;
            target.sky_other += r.sky_other;
            if r.sky_mismatch > 0 {
                println!(
                    "  chunk ({cx},{cz}): sky {} (+1: {}, other: {}) block {}",
                    r.sky_mismatch, r.sky_plus1, r.sky_other, r.block_mismatch
                );
            }
            if target.sky_samples.len() < 64 {
                target.sky_samples.extend(
                    r.sky_samples
                        .iter()
                        .take(64 - target.sky_samples.len())
                        .cloned(),
                );
            }
            if target.block_samples.len() < 400 {
                target.block_samples.extend(
                    r.block_samples
                        .iter()
                        .map(|s| format!("({cx},{cz}) {s}"))
                        .take(400 - target.block_samples.len()),
                );
            }
        }
    }
    println!(
        "== full sweep: {chunks_full} chunks with complete ring, {chunks_edge} at load edge =="
    );
    println!(
        "[complete-ring] sky: {}/{} ({:.4}%) | of which +1-bright: {} other: {} | block: {}/{} ({:.4}%)",
        full_ring.sky_mismatch,
        full_ring.sky_total,
        full_ring.sky_mismatch as f64 / full_ring.sky_total.max(1) as f64 * 100.0,
        full_ring.sky_plus1,
        full_ring.sky_other,
        full_ring.block_mismatch,
        full_ring.block_total,
        full_ring.block_mismatch as f64 / full_ring.block_total.max(1) as f64 * 100.0
    );
    for s in &full_ring.sky_samples {
        println!("  sky {s}");
    }
    println!(
        "[load-edge] sky: {}/{} ({:.4}%) | block: {}/{}",
        edge.sky_mismatch,
        edge.sky_total,
        edge.sky_mismatch as f64 / edge.sky_total.max(1) as f64 * 100.0,
        edge.block_mismatch,
        edge.block_total
    );
    for s in &full_ring.block_samples {
        println!("  block {s}");
    }
}
