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

    fn preliminary_surface_height(
        &self,
        x: i32,
        z: i32,
        profile: &vanilla_noise::OverworldTerrainProfile,
    ) -> i32 {
        self.terrain_noise.preliminary_surface_height(profile, x, z)
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.terrain_noise.biome(x, y, z)
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

    fn recompute_first_available_heights_accelerated(
        &mut self,
        min_y: i32,
        height: i32,
        gpu: Option<&gpu_worldgen::GpuWorldgenEngine>,
    ) {
        if let Some(gpu) = gpu
            && height as usize * HEIGHTMAP_ENTRY_COUNT == CHUNK_DAMPENING_LEN
        {
            match gpu.first_available_heights(&self.occupied_mask(height)) {
                Ok(heights) if heights.len() == HEIGHTMAP_ENTRY_COUNT => {
                    for (column, height) in self.columns.iter_mut().zip(heights) {
                        column.first_available_height = height;
                    }
                    return;
                }
                Ok(heights) => {
                    log::warn!(
                        "GPU 世界生成后处理返回高度图长度异常，已回退 CPU: expected={}, got={}",
                        HEIGHTMAP_ENTRY_COUNT,
                        heights.len()
                    );
                }
                Err(err) => {
                    log::warn!("GPU 世界生成后处理失败，已回退 CPU: {err:#}");
                }
            }
        }

        self.recompute_first_available_heights(min_y, height);
    }

    fn occupied_mask(&self, height: i32) -> Vec<u32> {
        let mut mask = vec![0_u32; height as usize * HEIGHTMAP_ENTRY_COUNT];
        for z in 0..16 {
            for x in 0..16 {
                for (y, layer) in self.column(x, z).blocks.iter().enumerate() {
                    if !layer.is_air {
                        mask[y * HEIGHTMAP_ENTRY_COUNT + z * 16 + x] = 1;
                    }
                }
            }
        }
        mask
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
