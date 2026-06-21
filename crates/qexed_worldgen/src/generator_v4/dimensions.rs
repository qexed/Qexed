#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum NoiseDimension {
    Overworld,
    Nether,
    End,
}

impl NoiseDimension {
    fn from_name(dimension: &str) -> Option<Self> {
        match dimension {
            "minecraft:overworld" => Some(Self::Overworld),
            "minecraft:the_nether" => Some(Self::Nether),
            "minecraft:the_end" => Some(Self::End),
            _ => None,
        }
    }

    fn biome(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:plains",
            Self::Nether => "minecraft:nether_wastes",
            Self::End => "minecraft:the_end",
        }
    }
}

#[derive(Debug, Clone)]
struct BasicDimensionLayers {
    air: BlockLayer,
    bedrock: BlockLayer,
    netherrack: BlockLayer,
    nether_quartz_ore: BlockLayer,
    nether_gold_ore: BlockLayer,
    ancient_debris: BlockLayer,
    magma_block: BlockLayer,
    soul_sand: BlockLayer,
    gravel: BlockLayer,
    blackstone: BlockLayer,
    lava: BlockLayer,
    end_stone: BlockLayer,
    obsidian: BlockLayer,
}

impl BasicDimensionLayers {
    fn new() -> Self {
        Self {
            air: BlockLayer::new("minecraft:air"),
            bedrock: BlockLayer::new("minecraft:bedrock"),
            netherrack: BlockLayer::new("minecraft:netherrack"),
            nether_quartz_ore: BlockLayer::new("minecraft:nether_quartz_ore"),
            nether_gold_ore: BlockLayer::new("minecraft:nether_gold_ore"),
            ancient_debris: BlockLayer::new("minecraft:ancient_debris"),
            magma_block: BlockLayer::new("minecraft:magma_block"),
            soul_sand: BlockLayer::new("minecraft:soul_sand"),
            gravel: BlockLayer::new("minecraft:gravel"),
            blackstone: BlockLayer::new("minecraft:blackstone"),
            lava: BlockLayer::new("minecraft:lava"),
            end_stone: BlockLayer::new("minecraft:end_stone"),
            obsidian: BlockLayer::new("minecraft:obsidian"),
        }
    }
}

fn basic_dimension_chunk(
    dimension: NoiseDimension,
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> NoiseChunkBlocks {
    let layers = BasicDimensionLayers::new();
    let columns = (0..HEIGHTMAP_ENTRY_COUNT)
        .into_par_iter()
        .map(|column| {
            let local_x = column % 16;
            let local_z = column / 16;
            let world_x = chunk_x * 16 + local_x as i32;
            let world_z = chunk_z * 16 + local_z as i32;
            let blocks = (WORLD_MIN_Y..=WORLD_MAX_Y)
                .map(|world_y| {
                    basic_dimension_layer(dimension, seed, world_x, world_y, world_z, &layers)
                })
                .collect::<Vec<_>>();
            let first_available_height = first_available_height_from_layers(&blocks);

            NoiseColumnBlocks {
                blocks,
                first_available_height,
            }
        })
        .collect();

    NoiseChunkBlocks {
        columns,
        biomes: uniform_dimension_biomes(dimension.biome()),
        block_entities: Vec::new(),
    }
}

fn basic_dimension_block_state_at(
    dimension: NoiseDimension,
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
) -> Option<i32> {
    if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&y) {
        return None;
    }

    let layers = BasicDimensionLayers::new();
    let layer = basic_dimension_layer(dimension, seed, x, y, z, &layers);
    (!layer.is_air).then_some(layer.block_state_id)
}

fn basic_dimension_layer(
    dimension: NoiseDimension,
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    layers: &BasicDimensionLayers,
) -> BlockLayer {
    match dimension {
        NoiseDimension::Overworld => layers.air.clone(),
        NoiseDimension::Nether => basic_nether_layer(seed, x, y, z, layers),
        NoiseDimension::End => basic_end_layer(seed, x, y, z, layers),
    }
}

fn basic_nether_layer(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    layers: &BasicDimensionLayers,
) -> BlockLayer {
    if !(0..=127).contains(&y) {
        return layers.air.clone();
    }

    if is_nether_bedrock(seed, x, y, z) {
        return layers.bedrock.clone();
    }

    if let Some(layer) = nether_patch_layer(seed, x, y, z, layers) {
        return layer;
    }

    if let Some(layer) = nether_ore_layer(seed, x, y, z, layers) {
        return layer;
    }

    if y <= 31 && nether_lava_sea_reaches(seed, x, y, z) {
        return layers.lava.clone();
    }

    if nether_cave_density(seed, x, y, z) > 0.58 {
        return layers.air.clone();
    }

    layers.netherrack.clone()
}

fn is_nether_bedrock(seed: i64, x: i32, y: i32, z: i32) -> bool {
    if y == 0 || y == 127 {
        return true;
    }

    if (1..=4).contains(&y) {
        return hash_chance(seed, x, y, z, 5) >= y;
    }

    if (123..=126).contains(&y) {
        return hash_chance(seed, x, y, z, 5) >= 127 - y;
    }

    false
}

fn nether_lava_sea_reaches(seed: i64, x: i32, y: i32, z: i32) -> bool {
    y <= 24
        || value_noise_3d(seed ^ 0x4f1b_6d2a_11c3_7795, x, y, z, 36) > -0.18
        || value_noise_2d(seed ^ 0x2249_579f_0d7d_21f3, x, z, 48) > 0.22
}

fn nether_cave_density(seed: i64, x: i32, y: i32, z: i32) -> f64 {
    let broad = value_noise_3d(seed ^ 0x37ac_09f4_5d21_8bb1, x, y, z, 42);
    let detail = value_noise_3d(seed ^ 0x6957_19af_f0c3_238d, x, y * 2, z, 22);
    let vertical_opening = if (24..=108).contains(&y) { 0.12 } else { -0.16 };
    broad * 0.72 + detail * 0.28 + vertical_opening
}

fn nether_patch_layer(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    layers: &BasicDimensionLayers,
) -> Option<BlockLayer> {
    if y <= 34 && value_noise_3d(seed ^ 0x72b9_c42d_9a0e_114f, x, y, z, 16) > 0.62 {
        return Some(layers.magma_block.clone());
    }

    if (28..=70).contains(&y) {
        let soul_noise = value_noise_3d(seed ^ 0x4c91_1b67_6eef_42a9, x, y / 2, z, 34);
        if soul_noise > 0.68 {
            return Some(layers.soul_sand.clone());
        }
        if soul_noise < -0.70 {
            return Some(layers.gravel.clone());
        }
    }

    if y <= 22 && value_noise_3d(seed ^ 0x0d6e_8c2a_7f31_d497, x, y, z, 28) > 0.48 {
        return Some(layers.blackstone.clone());
    }

    None
}

fn nether_ore_layer(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    layers: &BasicDimensionLayers,
) -> Option<BlockLayer> {
    let ore_hash = hash_u64(seed ^ 0x6dd6_2c4a_0d47_bf53, x, y, z);
    if (8..=22).contains(&y) && ore_hash.is_multiple_of(149) {
        return Some(layers.ancient_debris.clone());
    }
    if (23..=119).contains(&y) && ore_hash.is_multiple_of(557) {
        return Some(layers.ancient_debris.clone());
    }
    if (10..=117).contains(&y)
        && value_noise_3d(seed ^ 0x51a5_bed1_8c42_3e19, x, y, z, 12) > 0.74
        && ore_hash.is_multiple_of(17)
    {
        return Some(layers.nether_gold_ore.clone());
    }
    if (10..=118).contains(&y)
        && value_noise_3d(seed ^ 0x09e3_14a8_33dd_c8bb, x, y, z, 14) > 0.58
        && ore_hash.is_multiple_of(11)
    {
        return Some(layers.nether_quartz_ore.clone());
    }

    None
}

fn basic_end_layer(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    layers: &BasicDimensionLayers,
) -> BlockLayer {
    if let Some(layer) = end_obsidian_pillar_layer(seed, x, y, z, layers) {
        return layer;
    }

    let density = end_main_island_density(seed, x, y, z)
        .max(end_outer_island_density(seed, x, y, z));
    if density > 0.0 {
        layers.end_stone.clone()
    } else {
        layers.air.clone()
    }
}

fn end_main_island_density(seed: i64, x: i32, y: i32, z: i32) -> f64 {
    let distance = ((x as f64).powi(2) + (z as f64).powi(2)).sqrt();
    let radius = 112.0;
    let falloff = 1.0 - (distance / radius).powi(2);
    if falloff <= 0.0 {
        return -1.0;
    }

    let noise = value_noise_2d(seed ^ 0x56d9_93f2_daa0_771b, x, z, 28) * 3.0;
    let center_y = 62.0 - distance * 0.08 + noise;
    let half_height = 6.0 + falloff * 28.0;
    half_height - (y as f64 - center_y).abs()
}

fn end_outer_island_density(seed: i64, x: i32, y: i32, z: i32) -> f64 {
    let distance_from_origin = ((x as f64).powi(2) + (z as f64).powi(2)).sqrt();
    if distance_from_origin < 768.0 {
        return -1.0;
    }

    let mut best = -1.0;
    let cell_x = x.div_euclid(256);
    let cell_z = z.div_euclid(256);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let cx = cell_x + dx;
            let cz = cell_z + dz;
            let hash = hash_u64(seed ^ 0x2d8b_f173_64de_3bb7, cx, 0, cz);
            if hash % 5 != 0 {
                continue;
            }

            let center_x = cx * 256 + 64 + ((hash >> 8) % 128) as i32;
            let center_z = cz * 256 + 64 + ((hash >> 24) % 128) as i32;
            let radius = 28.0 + ((hash >> 40) % 34) as f64;
            let distance =
                (((x - center_x) as f64).powi(2) + ((z - center_z) as f64).powi(2)).sqrt();
            let falloff = 1.0 - (distance / radius).powi(2);
            if falloff <= 0.0 {
                continue;
            }

            let center_y = 62.0 + ((hash >> 56) % 9) as f64 - 4.0;
            let half_height = 3.0 + falloff * 12.0;
            let density = half_height - (y as f64 - center_y).abs();
            if density > best {
                best = density;
            }
        }
    }
    best
}

const END_PILLAR_CENTERS: [(i32, i32); 10] = [
    (42, 0),
    (34, 25),
    (13, 40),
    (-13, 40),
    (-34, 25),
    (-42, 0),
    (-34, -25),
    (-13, -40),
    (13, -40),
    (34, -25),
];

fn end_obsidian_pillar_layer(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    layers: &BasicDimensionLayers,
) -> Option<BlockLayer> {
    for (index, (center_x, center_z)) in END_PILLAR_CENTERS.iter().copied().enumerate() {
        let radius = 2 + index as i32 % 4;
        let height =
            76 + index as i32 * 3 + hash_chance(seed, center_x, index as i32, center_z, 12);
        if y < 40 || y > height {
            continue;
        }

        let dx = x - center_x;
        let dz = z - center_z;
        if dx * dx + dz * dz <= radius * radius {
            return Some(layers.obsidian.clone());
        }
    }

    None
}

fn first_available_height_from_layers(blocks: &[BlockLayer]) -> i32 {
    blocks
        .iter()
        .rposition(|layer| !layer.is_air)
        .map(|index| index as i32 + 1)
        .unwrap_or(0)
}

fn uniform_dimension_biomes(biome: &'static str) -> Vec<&'static str> {
    vec![biome; section_count() as usize * 64]
}

fn value_noise_2d(seed: i64, x: i32, z: i32, scale: i32) -> f64 {
    value_noise_3d(seed, x, 0, z, scale)
}

fn value_noise_3d(seed: i64, x: i32, y: i32, z: i32, scale: i32) -> f64 {
    let x0 = x.div_euclid(scale);
    let y0 = y.div_euclid(scale);
    let z0 = z.div_euclid(scale);
    let fx = smoothstep(x.rem_euclid(scale) as f64 / scale as f64);
    let fy = smoothstep(y.rem_euclid(scale) as f64 / scale as f64);
    let fz = smoothstep(z.rem_euclid(scale) as f64 / scale as f64);

    let c000 = hash_unit(seed, x0, y0, z0);
    let c100 = hash_unit(seed, x0 + 1, y0, z0);
    let c010 = hash_unit(seed, x0, y0 + 1, z0);
    let c110 = hash_unit(seed, x0 + 1, y0 + 1, z0);
    let c001 = hash_unit(seed, x0, y0, z0 + 1);
    let c101 = hash_unit(seed, x0 + 1, y0, z0 + 1);
    let c011 = hash_unit(seed, x0, y0 + 1, z0 + 1);
    let c111 = hash_unit(seed, x0 + 1, y0 + 1, z0 + 1);

    let x00 = lerp(c000, c100, fx);
    let x10 = lerp(c010, c110, fx);
    let x01 = lerp(c001, c101, fx);
    let x11 = lerp(c011, c111, fx);
    let y0 = lerp(x00, x10, fy);
    let y1 = lerp(x01, x11, fy);
    lerp(y0, y1, fz)
}

fn hash_unit(seed: i64, x: i32, y: i32, z: i32) -> f64 {
    let value = hash_u64(seed, x, y, z);
    (value as f64 / u64::MAX as f64) * 2.0 - 1.0
}

fn hash_chance(seed: i64, x: i32, y: i32, z: i32, bound: i32) -> i32 {
    (hash_u64(seed, x, y, z) % bound as u64) as i32
}

fn hash_u64(seed: i64, x: i32, y: i32, z: i32) -> u64 {
    let mut value = seed as u64;
    value ^= (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= (y as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= (z as i64 as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp(left: f64, right: f64, amount: f64) -> f64 {
    left + (right - left) * amount
}
