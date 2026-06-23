#[derive(Debug, Clone)]
struct TerrainDensity {
    blended_noise: vanilla_noise::BlendedNoise,
    terrain_noise: vanilla_noise::OverworldTerrainNoise,
}

const TERRAIN_NOISE_CELL_WIDTH: i32 = 4;
const TERRAIN_NOISE_CELL_HEIGHT: i32 = 8;

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

    fn preliminary_surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        self.terrain_noise.preliminary_surface_height(profile, x, z)
    }

    fn biome_at_block(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.terrain_noise.biome_at_block(x, y, z)
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.biome_at_block(x, y, z)
    }

    fn biome_at_quart(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.terrain_noise.biome_at_quart(x, y, z)
    }

    fn surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        let density_column = self.column_sampler(x, z, profile);
        (-64..=320)
            .rev()
            .find(|y| density_column.sample(*y) > 0.0)
            .unwrap_or(-64)
    }

    fn sample_with_profile(
        &self,
        x: i32,
        y: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> f64 {
        let base_3d = self.blended_noise.compute(x, y, z);
        self.terrain_noise.final_density(profile, x, y, z, base_3d)
    }

    fn chunk_density_cache(
        &self,
        chunk_min_x: i32,
        chunk_min_z: i32,
        min_y: i32,
        height: i32,
    ) -> Vec<ColumnDensityCache> {
        let cell_count_xz = 16 / TERRAIN_NOISE_CELL_WIDTH;
        let cell_count_y = height / TERRAIN_NOISE_CELL_HEIGHT;
        let lattice_xz = cell_count_xz + 1;
        let lattice_y = cell_count_y + 1;
        let mut lattice =
            vec![0.0; (lattice_xz * lattice_xz * lattice_y) as usize];

        for cell_z in 0..lattice_xz {
            for cell_x in 0..lattice_xz {
                let x = chunk_min_x + cell_x * TERRAIN_NOISE_CELL_WIDTH;
                let z = chunk_min_z + cell_z * TERRAIN_NOISE_CELL_WIDTH;
                let profile = self.profile(x, z);
                let density_column = self.column_sampler(x, z, &profile);
                for cell_y in 0..lattice_y {
                    let y = min_y + cell_y * TERRAIN_NOISE_CELL_HEIGHT;
                    lattice[noise_lattice_index(
                        cell_x,
                        cell_y,
                        cell_z,
                        lattice_xz,
                        lattice_y,
                    )] = density_column.sample(y);
                }
            }
        }

        (0..16 * 16)
            .map(|column| {
                let x = column % 16;
                let z = column / 16;
                let mut densities = Vec::with_capacity(height as usize);
                for y in min_y..min_y + height {
                    densities.push(interpolate_noise_lattice(
                        &lattice,
                        x,
                        y - min_y,
                        z,
                        lattice_xz,
                        lattice_y,
                    ));
                }

                let surface_height = densities
                    .iter()
                    .rposition(|density| *density > 0.0)
                    .map(|index| min_y + index as i32)
                    .unwrap_or(min_y)
                    .clamp(min_y + 1, min_y + height - 1);

                ColumnDensityCache {
                    surface_height,
                    densities,
                }
            })
            .collect()
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

fn noise_lattice_index(
    cell_x: i32,
    cell_y: i32,
    cell_z: i32,
    lattice_xz: i32,
    lattice_y: i32,
) -> usize {
    ((cell_z * lattice_xz + cell_x) * lattice_y + cell_y) as usize
}

fn interpolate_noise_lattice(
    lattice: &[f64],
    local_x: i32,
    local_y: i32,
    local_z: i32,
    lattice_xz: i32,
    lattice_y: i32,
) -> f64 {
    let cell_x = local_x / TERRAIN_NOISE_CELL_WIDTH;
    let cell_y = local_y / TERRAIN_NOISE_CELL_HEIGHT;
    let cell_z = local_z / TERRAIN_NOISE_CELL_WIDTH;
    let factor_x =
        (local_x % TERRAIN_NOISE_CELL_WIDTH) as f64 / TERRAIN_NOISE_CELL_WIDTH as f64;
    let factor_y =
        (local_y % TERRAIN_NOISE_CELL_HEIGHT) as f64 / TERRAIN_NOISE_CELL_HEIGHT as f64;
    let factor_z =
        (local_z % TERRAIN_NOISE_CELL_WIDTH) as f64 / TERRAIN_NOISE_CELL_WIDTH as f64;

    let at = |dx, dy, dz| {
        lattice[noise_lattice_index(
            cell_x + dx,
            cell_y + dy,
            cell_z + dz,
            lattice_xz,
            lattice_y,
        )]
    };

    let y00 = terrain_density_lerp(factor_y, at(0, 0, 0), at(0, 1, 0));
    let y10 = terrain_density_lerp(factor_y, at(1, 0, 0), at(1, 1, 0));
    let y01 = terrain_density_lerp(factor_y, at(0, 0, 1), at(0, 1, 1));
    let y11 = terrain_density_lerp(factor_y, at(1, 0, 1), at(1, 1, 1));
    let x0 = terrain_density_lerp(factor_x, y00, y10);
    let x1 = terrain_density_lerp(factor_x, y01, y11);
    terrain_density_lerp(factor_z, x0, x1)
}

fn terrain_density_lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
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

#[derive(Debug, Clone)]
struct ColumnDensityCache {
    surface_height: i32,
    densities: Vec<f64>,
}

impl ColumnDensityCache {
    fn density_at(&self, y: i32, min_y: i32) -> f64 {
        let index = (y - min_y) as usize;
        self.densities[index]
    }
}

#[derive(Debug, Clone)]
struct NoiseChunkBlocks {
    columns: Vec<NoiseColumnBlocks>,
    biomes: Vec<&'static str>,
    block_entities: Vec<GeneratedBlockEntity>,
}

impl NoiseChunkBlocks {
    fn column(&self, x: usize, z: usize) -> &NoiseColumnBlocks {
        &self.columns[z * 16 + x]
    }

    fn column_mut(&mut self, x: usize, z: usize) -> &mut NoiseColumnBlocks {
        &mut self.columns[z * 16 + x]
    }

    fn layer(&self, x: usize, y: i32, z: usize, min_y: i32) -> Option<&BlockLayer> {
        let index = usize::try_from(y - min_y).ok()?;
        self.column(x, z).blocks.get(index)
    }

    fn surface_water_reaches(
        &self,
        x: usize,
        y: i32,
        z: usize,
        min_y: i32,
        sea_level: i32,
    ) -> bool {
        let Some(start) = usize::try_from(y - min_y).ok() else {
            return false;
        };
        self.surface_water_reaches_from(x, z, start, min_y, sea_level)
    }

    fn surface_water_reaches_above(
        &self,
        x: usize,
        y: i32,
        z: usize,
        min_y: i32,
        sea_level: i32,
    ) -> bool {
        let Some(start) = usize::try_from(y + 1 - min_y).ok() else {
            return false;
        };
        self.surface_water_reaches_from(x, z, start, min_y, sea_level)
    }

    fn adjacent_surface_water_reaches(
        &self,
        x: usize,
        y: i32,
        z: usize,
        min_y: i32,
        sea_level: i32,
    ) -> bool {
        [(1_isize, 0_isize), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .filter_map(|(dx, dz)| {
                let nx = x.checked_add_signed(dx)?;
                let nz = z.checked_add_signed(dz)?;
                (nx < 16 && nz < 16).then_some((nx, nz))
            })
            .any(|(nx, nz)| self.surface_water_reaches(nx, y, nz, min_y, sea_level))
    }

    fn surface_water_reaches_from(
        &self,
        x: usize,
        z: usize,
        start: usize,
        min_y: i32,
        sea_level: i32,
    ) -> bool {
        let column = self.column(x, z);
        let Some(sea_index) = usize::try_from(sea_level - 1 - min_y).ok() else {
            return false;
        };
        let end = sea_index.min(column.blocks.len().saturating_sub(1));
        if start > end {
            return false;
        }

        let mut has_water = false;
        for layer in &column.blocks[start..=end] {
            if is_water_layer(layer) {
                has_water = true;
            } else if !layer.is_air {
                return false;
            }
        }
        has_water
    }

    fn set_layer(&mut self, x: usize, y: i32, z: usize, min_y: i32, layer: BlockLayer) {
        if let Ok(index) = usize::try_from(y - min_y) {
            let is_air = layer.is_air;
            if !layer.is("minecraft:chest") && !layer.is("minecraft:spawner") {
                self.remove_block_entity_by_local(x, y, z);
            }
            let column = self.column_mut(x, z);
            if let Some(block) = column.blocks.get_mut(index) {
                *block = layer;
                update_first_available_height(column, index, is_air);
            }
        }
    }

    fn push_block_entity(
        &mut self,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        entity_type: i32,
        nbt: Tag,
    ) {
        self.block_entities
            .retain(|entity| entity.position != (world_x, world_y, world_z));
        self.block_entities.push(GeneratedBlockEntity {
            position: (world_x, world_y, world_z),
            entity_type,
            nbt,
        });
    }

    fn remove_block_entity_by_local(&mut self, local_x: usize, world_y: i32, local_z: usize) {
        self.block_entities.retain(|entity| {
            entity.position.1 != world_y
                || entity.position.0.rem_euclid(16) as usize != local_x
                || entity.position.2.rem_euclid(16) as usize != local_z
        });
    }

    fn block_entities_as_packet(&self, chunk_x: i32, chunk_z: i32) -> Vec<BlockEntities> {
        let chunk_min_x = chunk_x * 16;
        let chunk_min_z = chunk_z * 16;
        let mut entities = self
            .block_entities
            .iter()
            .filter_map(|entity| {
                let local_x = local_coord(entity.position.0, chunk_min_x)?;
                let local_z = local_coord(entity.position.2, chunk_min_z)?;
                Some(BlockEntities {
                    xz: ((local_x as u8) << 4) | local_z as u8,
                    y: entity.position.1 as u16,
                    entity_type: VarInt(entity.entity_type),
                    nbt: OptionalNbt(Some(entity.nbt.clone())),
                })
            })
            .collect::<Vec<_>>();
        entities.sort_by_key(|entity| (entity.y, entity.xz));
        entities
    }

    fn biome(&self, section_y: i32, x: usize, y: usize, z: usize) -> &'static str {
        let section_index = (section_y - WORLD_MIN_SECTION_Y) as usize;
        self.biomes[((section_index * 4 + x) * 4 + y) * 4 + z]
    }

    fn ocean_floor_wg_height(&self, x: usize, z: usize, min_y: i32) -> i32 {
        let column = self.column(x, z);
        let top = column
            .first_available_height
            .clamp(0, column.blocks.len() as i32) as usize;
        column
            .blocks
            .get(..top)
            .unwrap_or(&column.blocks)
            .iter()
            .rposition(is_full_solid_layer)
            .map(|index| min_y + index as i32 + 1)
            .unwrap_or(min_y)
    }

    fn world_surface_wg_height(&self, x: usize, z: usize, min_y: i32) -> i32 {
        min_y + self.column(x, z).first_available_height
    }

    fn recompute_first_available_heights(&mut self, min_y: i32, height: i32) {
        for column in &mut self.columns {
            let highest = column
                .blocks
                .iter()
                .rposition(|layer| !layer.is_air)
                .map(|index| min_y + index as i32 + 1)
                .unwrap_or(min_y);
            column.first_available_height = (highest - min_y).clamp(0, height);
        }
    }
}

fn update_first_available_height(column: &mut NoiseColumnBlocks, index: usize, is_air: bool) {
    let height = index as i32 + 1;
    if is_air {
        if column.first_available_height == height {
            column.first_available_height = column
                .blocks
                .iter()
                .rposition(|layer| !layer.is_air)
                .map(|index| index as i32 + 1)
                .unwrap_or(0);
        }
    } else if height > column.first_available_height {
        column.first_available_height = height;
    }
}

#[derive(Debug, Clone)]
struct GeneratedBlockEntity {
    position: (i32, i32, i32),
    entity_type: i32,
    nbt: Tag,
}

#[derive(Debug, Clone)]
struct NoiseColumnBlocks {
    blocks: Vec<BlockLayer>,
    first_available_height: i32,
}
