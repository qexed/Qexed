use anyhow::Result;
use qexed_nbt::Tag;
use serde::Deserialize;

use crate::{
    cache::WorldgenCache,
    chunk_nbt::{chunk_root, ocean_floor_height},
    constants::{SEA_LEVEL, SECTION_HEIGHT, WORLD_HEIGHT, WORLD_MIN_Y},
    empty::empty_chunk_root,
    generator_v4,
    registry::{
        BlockIds, BlockStateJson, SurfaceBlockIds, block_state_id, qexed_registry_block_id,
    },
    types::{ChunkRequest, WorldGenerator},
    util::{column_index, lerp, normalize_dimension, read_json, smoothstep},
    vanilla_noise,
};
#[derive(Debug, Clone, Deserialize)]
struct NoiseSettings {
    #[serde(default)]
    default_block: BlockStateJson,
    #[serde(default)]
    default_fluid: BlockStateJson,
    #[serde(default)]
    sea_level: Option<i32>,
    noise: NoiseShape,
}

#[derive(Debug, Clone, Deserialize)]
struct NoiseShape {
    min_y: i32,
    height: i32,
}

impl WorldGenerator {
    pub fn new(cache: WorldgenCache, seed: i64) -> Result<Self> {
        cache.ensure_ready()?;
        Ok(Self { cache, seed })
    }

    pub fn default_cache(seed: i64) -> Result<Self> {
        Self::new(WorldgenCache::default(), seed)
    }

    pub fn generate_chunk_nbt(&self, request: ChunkRequest<'_>) -> Result<Tag> {
        match normalize_dimension(request.dimension).as_str() {
            "minecraft:overworld" => self.generate_overworld(request.chunk_x, request.chunk_z),
            "minecraft:the_nether" => self.generate_basic_dimension(
                request.chunk_x,
                request.chunk_z,
                "minecraft:netherrack",
                "minecraft:lava",
                "minecraft:nether_wastes",
                31,
            ),
            "minecraft:the_end" => self.generate_basic_dimension(
                request.chunk_x,
                request.chunk_z,
                "minecraft:end_stone",
                "minecraft:air",
                "minecraft:the_end",
                0,
            ),
            other => {
                log::warn!("unknown worldgen dimension, generating empty chunk: {other}");
                Ok(empty_chunk_root(request.chunk_x, request.chunk_z))
            }
        }
    }

    fn generate_overworld(&self, chunk_x: i32, chunk_z: i32) -> Result<Tag> {
        match generator_v4::generate_overworld_chunk_nbt(self.seed, chunk_x, chunk_z) {
            Ok(chunk) => return Ok(chunk),
            Err(err) => {
                log::warn!(
                    "v4 vanilla worldgen pipeline failed, falling back to column generator: {err:#}"
                );
            }
        }

        let settings = self.load_noise_settings("overworld")?;
        let min_y = settings.noise.min_y;
        let height = settings.noise.height;
        let sea_level = settings.sea_level.unwrap_or(SEA_LEVEL);
        let ids = BlockIds::from_cache()?;
        let surface_ids = SurfaceBlockIds::from_cache()?;
        let default_block = block_state_id(&settings.default_block)?;
        let default_fluid = block_state_id(&settings.default_fluid)?;
        let terrain_density =
            TerrainDensity::overworld(self.seed, vanilla_noise::OverworldNoiseKind::Default);
        let surface_rules = vanilla_noise::OverworldSurfaceRules::new(self.seed);
        let aquifer = vanilla_noise::OverworldAquifer::new(self.seed, sea_level);
        let ore_veins = vanilla_noise::OreVeinNoise::new(self.seed);

        let section_count = (height / SECTION_HEIGHT) as usize;
        let mut columns = vec![vec![ids.air; height as usize]; 16 * 16];
        let mut biomes = vec!["minecraft:plains"; section_count * 4 * 4 * 4];
        let mut heightmap = vec![min_y; 16 * 16];
        let mut ocean_floor_heightmap = vec![min_y; 16 * 16];
        for section in 0..section_count {
            let section_y = min_y / SECTION_HEIGHT + section as i32;
            for biome_x in 0..4 {
                for biome_y in 0..4 {
                    for biome_z in 0..4 {
                        let sample_x = chunk_x * 16 + biome_x as i32 * 4;
                        let sample_y = section_y * SECTION_HEIGHT + biome_y as i32 * 4;
                        let sample_z = chunk_z * 16 + biome_z as i32 * 4;
                        biomes[((section * 4 + biome_x) * 4 + biome_y) * 4 + biome_z] =
                            terrain_density.biome(sample_x, sample_y, sample_z);
                    }
                }
            }
        }
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = chunk_x * 16 + local_x;
                let world_z = chunk_z * 16 + local_z;
                let profile = terrain_density.profile(world_x, world_z);
                let terrain = terrain_density.surface_height(world_x, world_z, &profile);
                let preliminary_surface =
                    terrain_density.preliminary_surface_height(world_x, world_z, &profile);
                let biome = terrain_density.biome(world_x, terrain, world_z);
                let slope = terrain_density.surface_slope(world_x, world_z, terrain);
                let index = column_index(local_x, local_z);
                heightmap[index] = terrain + 1;

                let density_column = terrain_density.column_sampler(world_x, world_z, &profile);
                let water_height = water_height(
                    min_y,
                    height,
                    sea_level,
                    world_x,
                    world_z,
                    terrain,
                    preliminary_surface,
                    &density_column,
                    &aquifer,
                );
                for y in min_y..min_y + height {
                    let density = density_column.sample(y);
                    let block = if y <= min_y {
                        surface_ids.bedrock
                    } else if density > 0.0 {
                        if surface_rules.is_bedrock_floor(world_x, y, world_z, min_y) {
                            surface_ids.bedrock
                        } else if y < terrain - 8 {
                            default_terrain_block(
                                &surface_ids,
                                &surface_rules,
                                &ore_veins,
                                world_x,
                                y,
                                world_z,
                                default_block,
                            )
                        } else {
                            let context = vanilla_noise::SurfaceRuleContext {
                                x: world_x,
                                y,
                                z: world_z,
                                surface_height: terrain,
                                above_water: water_height.is_none_or(|height| y >= height),
                                sea_level,
                                min_y,
                                biome,
                                slope,
                            };
                            surface_rules
                                .block_at_with_preliminary_surface(context, preliminary_surface)
                                .map(|block| surface_ids.id(block))
                                .unwrap_or_else(|| {
                                    default_terrain_block(
                                        &surface_ids,
                                        &surface_rules,
                                        &ore_veins,
                                        world_x,
                                        y,
                                        world_z,
                                        default_block,
                                    )
                                })
                        }
                    } else if y < sea_level && y > terrain {
                        default_fluid
                    } else {
                        match aquifer.substance_at(
                            world_x,
                            y,
                            world_z,
                            density,
                            preliminary_surface,
                        ) {
                            vanilla_noise::AquiferSubstance::DefaultBlock => {
                                ore_vein_block(&surface_ids, &ore_veins, world_x, y, world_z)
                                    .unwrap_or(default_block)
                            }
                            vanilla_noise::AquiferSubstance::Fluid(
                                vanilla_noise::AquiferFluid::Air,
                            ) => ids.air,
                            vanilla_noise::AquiferSubstance::Fluid(
                                vanilla_noise::AquiferFluid::Water,
                            ) => default_fluid,
                            vanilla_noise::AquiferSubstance::Fluid(
                                vanilla_noise::AquiferFluid::Lava,
                            ) => surface_ids.lava,
                        }
                    };
                    columns[index][(y - min_y) as usize] = block;
                }
                ocean_floor_heightmap[index] = ocean_floor_height(min_y, &columns[index], &ids);
            }
        }

        Ok(chunk_root(
            chunk_x,
            chunk_z,
            min_y,
            height,
            &columns,
            &biomes,
            &heightmap,
            &ocean_floor_heightmap,
        )?)
    }

    fn generate_basic_dimension(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        solid_block: &str,
        fluid_block: &str,
        biome: &str,
        fluid_level: i32,
    ) -> Result<Tag> {
        let min_y = WORLD_MIN_Y;
        let height = WORLD_HEIGHT;
        let solid = qexed_registry_block_id(solid_block)?;
        let fluid = qexed_registry_block_id(fluid_block)?;
        let air = qexed_registry_block_id("minecraft:air")?;

        let mut columns = vec![vec![air; height as usize]; 16 * 16];
        let ids = BlockIds::from_cache()?;
        let mut heightmap = vec![min_y; 16 * 16];
        let mut ocean_floor_heightmap = vec![min_y; 16 * 16];
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = chunk_x * 16 + local_x;
                let world_z = chunk_z * 16 + local_z;
                let surface = basic_dimension_height(self.seed, world_x, world_z);
                let index = column_index(local_x, local_z);
                heightmap[index] = surface + 1;
                for y in min_y..min_y + height {
                    columns[index][(y - min_y) as usize] = if y <= surface {
                        solid
                    } else if fluid != air && y <= fluid_level {
                        fluid
                    } else {
                        air
                    };
                }
                ocean_floor_heightmap[index] = ocean_floor_height(min_y, &columns[index], &ids);
            }
        }
        let section_count = (height / SECTION_HEIGHT) as usize;
        let biomes = vec![biome; section_count * 4 * 4 * 4];
        chunk_root(
            chunk_x,
            chunk_z,
            min_y,
            height,
            &columns,
            &biomes,
            &heightmap,
            &ocean_floor_heightmap,
        )
    }

    fn load_noise_settings(&self, name: &str) -> Result<NoiseSettings> {
        let path = self
            .cache
            .data_root()
            .join("worldgen/noise_settings")
            .join(format!("{name}.json"));
        read_json(&path)
    }
}

#[derive(Debug, Clone)]
struct TerrainDensity {
    blended_noise: vanilla_noise::BlendedNoise,
    terrain_noise: vanilla_noise::OverworldTerrainNoise,
}

impl TerrainDensity {
    fn overworld(seed: i64, noise_kind: vanilla_noise::OverworldNoiseKind) -> Self {
        Self {
            blended_noise: vanilla_noise::BlendedNoise::overworld(seed),
            terrain_noise: vanilla_noise::OverworldTerrainNoise::with_kind(seed, noise_kind),
        }
    }

    fn profile(&self, x: i32, z: i32) -> vanilla_noise::OverworldTerrainProfile {
        self.terrain_noise.profile(x, z)
    }

    fn surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        let density_column = self.column_sampler(x, z, profile);
        (WORLD_MIN_Y..WORLD_MIN_Y + WORLD_HEIGHT)
            .rev()
            .find(|y| density_column.sample(*y) > 0.0)
            .unwrap_or(WORLD_MIN_Y)
    }

    fn preliminary_surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        self.terrain_noise.preliminary_surface_height(profile, x, z)
    }

    fn surface_slope(&self, x: i32, z: i32, center_height: i32) -> i32 {
        [(1, 0), (0, 1)]
            .into_iter()
            .map(|(dx, dz)| {
                let profile = self.profile(x + dx, z + dz);
                (self.surface_height(x + dx, z + dz, &profile) - center_height).abs()
            })
            .max()
            .unwrap_or_default()
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.terrain_noise.biome_at_block(x, y, z)
    }

    fn column_sampler<'a>(
        &'a self,
        x: i32,
        z: i32,
        profile: &'a vanilla_noise::OverworldTerrainProfile,
    ) -> TerrainDensityColumn<'a> {
        TerrainDensityColumn {
            blended_noise: self.blended_noise.column_sampler(x, z),
            terrain_noise: self.terrain_noise.column_sampler(profile, x, z),
        }
    }
}

struct TerrainDensityColumn<'a> {
    blended_noise: vanilla_noise::BlendedNoiseColumn<'a>,
    terrain_noise: vanilla_noise::OverworldTerrainColumn<'a>,
}

impl TerrainDensityColumn<'_> {
    fn sample(&self, y: i32) -> f64 {
        let base_3d = self.blended_noise.compute(y);
        self.terrain_noise.final_density(y, base_3d)
    }
}

fn water_height(
    min_y: i32,
    height: i32,
    sea_level: i32,
    x: i32,
    z: i32,
    surface_height: i32,
    preliminary_surface: i32,
    density_column: &TerrainDensityColumn<'_>,
    aquifer: &vanilla_noise::OverworldAquifer,
) -> Option<i32> {
    let start = surface_height
        .max(sea_level - 1)
        .saturating_add(1)
        .clamp(min_y, min_y + height - 1);
    (min_y..=start).rev().find(|y| {
        if *y < sea_level && *y > surface_height {
            return true;
        }

        let density = density_column.sample(*y);
        if density > 0.0 {
            return false;
        }
        matches!(
            aquifer.substance_at(x, *y, z, density, preliminary_surface),
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Water)
        )
    })
}

fn default_terrain_block(
    ids: &SurfaceBlockIds,
    surface_rules: &vanilla_noise::OverworldSurfaceRules,
    ore_veins: &vanilla_noise::OreVeinNoise,
    x: i32,
    y: i32,
    z: i32,
    default_block: i32,
) -> i32 {
    if surface_rules.is_deepslate(x, y, z) {
        ore_vein_block(ids, ore_veins, x, y, z).unwrap_or(ids.deepslate)
    } else {
        ore_vein_block(ids, ore_veins, x, y, z).unwrap_or(default_block)
    }
}

fn ore_vein_block(
    ids: &SurfaceBlockIds,
    ore_veins: &vanilla_noise::OreVeinNoise,
    x: i32,
    y: i32,
    z: i32,
) -> Option<i32> {
    Some(match ore_veins.block_at(x, y, z)? {
        vanilla_noise::OreVeinBlock::CopperOre => ids.copper_ore,
        vanilla_noise::OreVeinBlock::RawCopperBlock => ids.raw_copper_block,
        vanilla_noise::OreVeinBlock::Granite => ids.granite,
        vanilla_noise::OreVeinBlock::DeepslateIronOre => ids.deepslate_iron_ore,
        vanilla_noise::OreVeinBlock::RawIronBlock => ids.raw_iron_block,
        vanilla_noise::OreVeinBlock::Tuff => ids.tuff,
    })
}

fn basic_dimension_height(seed: i64, x: i32, z: i32) -> i32 {
    (48.0 + value_noise(seed ^ 0x34d0, x, z, 64.0) * 18.0).round() as i32
}

fn value_noise(seed: i64, x: i32, z: i32, scale: f64) -> f64 {
    let fx = x as f64 / scale;
    let fz = z as f64 / scale;
    let x0 = fx.floor() as i32;
    let z0 = fz.floor() as i32;
    let tx = smoothstep(fx - f64::from(x0));
    let tz = smoothstep(fz - f64::from(z0));
    let a = hash_unit(seed, x0, z0);
    let b = hash_unit(seed, x0 + 1, z0);
    let c = hash_unit(seed, x0, z0 + 1);
    let d = hash_unit(seed, x0 + 1, z0 + 1);
    lerp(lerp(a, b, tx), lerp(c, d, tx), tz)
}

fn hash_unit(seed: i64, x: i32, z: i32) -> f64 {
    let mut value = seed as u64;
    value ^= (x as u64).wrapping_mul(0x9e3779b97f4a7c15);
    value ^= (z as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    (value as f64 / u64::MAX as f64) * 2.0 - 1.0
}
