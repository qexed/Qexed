#[derive(Debug, Clone)]
struct PlacedSurfaceFeature {
    step_index: i32,
    feature_index: i32,
    count: OrePlacementCount,
    heightmap: SurfaceHeightmap,
    y_offset: i32,
    config: SurfaceFeatureConfig,
    biome_filter: FeatureBiomeFilter,
}

impl PlacedSurfaceFeature {
    fn forest_rock(feature_index: i32) -> Self {
        Self {
            step_index: 2,
            feature_index,
            count: OrePlacementCount::Constant(2),
            heightmap: SurfaceHeightmap::MotionBlocking,
            y_offset: 0,
            config: SurfaceFeatureConfig::BlockBlob(BlockBlobSurfaceConfig::forest_rock()),
            biome_filter: FeatureBiomeFilter::Include(FOREST_ROCK_BIOMES),
        }
    }

    fn ice_spike(feature_index: i32) -> Self {
        Self {
            step_index: 4,
            feature_index,
            count: OrePlacementCount::Constant(3),
            heightmap: SurfaceHeightmap::MotionBlocking,
            y_offset: 0,
            config: SurfaceFeatureConfig::IceSpike(IceSpikeSurfaceConfig::new()),
            biome_filter: FeatureBiomeFilter::Include(ICE_SPIKE_BIOMES),
        }
    }

    fn ice_patch(feature_index: i32) -> Self {
        Self {
            step_index: 4,
            feature_index,
            count: OrePlacementCount::Constant(2),
            heightmap: SurfaceHeightmap::MotionBlocking,
            y_offset: -1,
            config: SurfaceFeatureConfig::Disk(SurfaceDiskConfig::ice_patch()),
            biome_filter: FeatureBiomeFilter::Include(ICE_SPIKE_BIOMES),
        }
    }

    fn pale_moss_patch(feature_index: i32) -> Self {
        Self {
            step_index: 9,
            feature_index,
            count: OrePlacementCount::Constant(1),
            heightmap: SurfaceHeightmap::MotionBlockingNoLeaves,
            y_offset: 0,
            config: SurfaceFeatureConfig::VegetationPatch(VegetationPatchConfig::pale_moss_patch()),
            biome_filter: FeatureBiomeFilter::Include(PALE_MOSS_PATCH_BIOMES),
        }
    }

    fn iceberg_packed(feature_index: i32) -> Self {
        Self {
            step_index: 2,
            feature_index,
            count: OrePlacementCount::Rarity(16),
            heightmap: SurfaceHeightmap::SeaLevel,
            y_offset: 0,
            config: SurfaceFeatureConfig::Iceberg(IcebergSurfaceConfig::packed()),
            biome_filter: FeatureBiomeFilter::Include(ICEBERG_BIOMES),
        }
    }

    fn iceberg_blue(feature_index: i32) -> Self {
        Self {
            step_index: 2,
            feature_index,
            count: OrePlacementCount::Rarity(200),
            heightmap: SurfaceHeightmap::SeaLevel,
            y_offset: 0,
            config: SurfaceFeatureConfig::Iceberg(IcebergSurfaceConfig::blue()),
            biome_filter: FeatureBiomeFilter::Include(ICEBERG_BIOMES),
        }
    }

    fn blue_ice(feature_index: i32) -> Self {
        Self {
            step_index: 4,
            feature_index,
            count: OrePlacementCount::Uniform { min: 0, max: 19 },
            heightmap: SurfaceHeightmap::HeightRange(OreHeight::Uniform(
                HeightAnchor::Absolute(30),
                HeightAnchor::Absolute(61),
            )),
            y_offset: 0,
            config: SurfaceFeatureConfig::BlueIce(BlueIceSurfaceConfig::new()),
            biome_filter: FeatureBiomeFilter::Include(ICEBERG_BIOMES),
        }
    }

    fn place(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let Some((world_x, world_y, world_z)) =
                self.sample_origin(settings, origin_x, origin_z, chunk, random)
            else {
                continue;
            };
            self.config.place_resolved(
                settings, origin_x, origin_z, chunk, random, world_x, world_y, world_z,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        for _ in 0..self.count.sample(random) {
            let Some((world_x, world_y, world_z)) =
                self.sample_origin(settings, source_origin_x, source_origin_z, source_chunk, random)
            else {
                continue;
            };
            self.config.place_spillover_resolved(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            );
        }
    }

    fn sample_origin(
        &self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) -> Option<(i32, i32, i32)> {
        let world_x = origin_x + random.next_int(16);
        let world_z = origin_z + random.next_int(16);
        let (local_x, local_z) = local_coords(world_x, world_z, origin_x, origin_z)?;
        let world_y = self
            .heightmap
            .height(settings, chunk, local_x, local_z, random)
            + self.y_offset;
        if world_y <= settings.min_y
            || !self
                .biome_filter
                .allows_at(&settings.density, world_x, world_y, world_z)
        {
            return None;
        }
        let world_y =
            self.config
                .resolve_origin_y(settings, origin_x, origin_z, chunk, world_x, world_y, world_z)?;
        Some((world_x, world_y, world_z))
    }
}

#[derive(Debug, Clone, Copy)]
enum SurfaceHeightmap {
    MotionBlocking,
    MotionBlockingNoLeaves,
    SeaLevel,
    HeightRange(OreHeight),
}

impl SurfaceHeightmap {
    fn height(
        self,
        settings: &NoiseSettings,
        chunk: &NoiseChunkBlocks,
        local_x: usize,
        local_z: usize,
        random: &mut FeatureRandom,
    ) -> i32 {
        match self {
            Self::MotionBlocking => chunk.world_surface_wg_height(local_x, local_z, settings.min_y),
            Self::MotionBlockingNoLeaves => chunk
                .column(local_x, local_z)
                .blocks
                .iter()
                .rposition(|layer| !layer.is_air && !is_leaf_layer(layer))
                .map(|index| settings.min_y + index as i32 + 1)
                .unwrap_or(settings.min_y),
            Self::SeaLevel => settings.sea_level,
            Self::HeightRange(height) => height.sample(settings, random),
        }
    }
}

#[derive(Debug, Clone)]
enum SurfaceFeatureConfig {
    BlockBlob(BlockBlobSurfaceConfig),
    IceSpike(IceSpikeSurfaceConfig),
    Disk(SurfaceDiskConfig),
    VegetationPatch(VegetationPatchConfig),
    Iceberg(IcebergSurfaceConfig),
    BlueIce(BlueIceSurfaceConfig),
}

impl SurfaceFeatureConfig {
    #[allow(clippy::too_many_arguments)]
    fn resolve_origin_y(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        match self {
            Self::BlockBlob(config) => config.find_origin_y(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
            ),
            Self::IceSpike(config) => config.find_origin_y(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                world_x,
                world_y,
                world_z,
            ),
            Self::Disk(config) => config.resolve_origin_y(
                settings, chunk_min_x, chunk_min_z, chunk, world_x, world_y, world_z,
            ),
            Self::VegetationPatch(_) => Some(world_y),
            Self::Iceberg(_) => Some(settings.sea_level),
            Self::BlueIce(_) => Some(world_y),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        match self {
            Self::BlockBlob(config) => config.place_resolved(
                settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            ),
            Self::IceSpike(config) => config.place_resolved(
                settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            ),
            Self::Disk(config) => config.place(
                settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            ),
            Self::VegetationPatch(config) => config.place(
                settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            ),
            Self::Iceberg(config) => config.place(
                settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            ),
            Self::BlueIce(config) => config.place(
                settings, chunk_min_x, chunk_min_z, chunk, random, world_x, world_y, world_z,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover_resolved(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        match self {
            Self::BlockBlob(config) => config.place_with_spillover(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::IceSpike(config) => config.place_with_spillover(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::Iceberg(config) => config.place_with_spillover(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::BlueIce(config) => config.place_with_spillover(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::Disk(config) => config.place_with_spillover(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
            Self::VegetationPatch(config) => config.place_spillover(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                world_y,
                world_z,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct BlockBlobSurfaceConfig {
    block: BlockLayer,
}

impl BlockBlobSurfaceConfig {
    fn forest_rock() -> Self {
        Self {
            block: BlockLayer::new("minecraft:mossy_cobblestone"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn find_origin_y(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        mut world_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        while world_y > settings.min_y + 3
            && !layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            )
            .is_some_and(is_forest_rock_can_place_on_layer)
        {
            world_y -= 1;
        }

        (world_y > settings.min_y + 3).then_some(world_y)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let mut origin_y = world_y;
        let mut origin_x = world_x;
        let mut origin_z = world_z;
        let mut placed = false;

        for _ in 0..3 {
            let radius_x = random.next_int(2);
            let radius_y = random.next_int(2);
            let radius_z = random.next_int(2);
            let threshold = (radius_x + radius_y + radius_z) as f64 / 3.0 + 0.5;

            for x in origin_x - radius_x..=origin_x + radius_x {
                for y in origin_y - radius_y..=origin_y + radius_y {
                    for z in origin_z - radius_z..=origin_z + radius_z {
                        if squared_distance(x, y, z, origin_x, origin_y, origin_z)
                            <= threshold * threshold
                            && set_surface_block_at_world(
                                chunk,
                                chunk_min_x,
                                chunk_min_z,
                                x,
                                y,
                                z,
                                settings.min_y,
                                self.block.clone(),
                            )
                        {
                            placed = true;
                        }
                    }
                }
            }

            origin_x += -1 + random.next_int(2);
            origin_y -= random.next_int(2);
            origin_z += -1 + random.next_int(2);
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let mut origin_y = world_y;
        let mut origin_x = world_x;
        let mut origin_z = world_z;
        let mut placed = false;

        for _ in 0..3 {
            let radius_x = random.next_int(2);
            let radius_y = random.next_int(2);
            let radius_z = random.next_int(2);
            let threshold = (radius_x + radius_y + radius_z) as f64 / 3.0 + 0.5;

            for x in origin_x - radius_x..=origin_x + radius_x {
                for y in origin_y - radius_y..=origin_y + radius_y {
                    for z in origin_z - radius_z..=origin_z + radius_z {
                        if squared_distance(x, y, z, origin_x, origin_y, origin_z)
                            <= threshold * threshold
                            && set_surface_block_in_context(
                                source_chunk,
                                source_min_x,
                                source_min_z,
                                target_chunk,
                                target_min_x,
                                target_min_z,
                                x,
                                y,
                                z,
                                settings.min_y,
                                self.block.clone(),
                            )
                        {
                            placed = true;
                        }
                    }
                }
            }

            origin_x += -1 + random.next_int(2);
            origin_y -= random.next_int(2);
            origin_z += -1 + random.next_int(2);
        }

        placed
    }
}

#[derive(Debug, Clone)]
struct IceSpikeSurfaceConfig {
    block: BlockLayer,
}

impl IceSpikeSurfaceConfig {
    fn new() -> Self {
        Self {
            block: BlockLayer::new("minecraft:packed_ice"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn find_origin_y(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        mut world_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        while is_air_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) && world_y > settings.min_y + 2
        {
            world_y -= 1;
        }
        layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
        .is_some_and(|layer| layer.is("minecraft:snow_block"))
        .then_some(world_y)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_resolved(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let mut origin_y = world_y;
        origin_y += random.next_int(4);
        let height = random.next_int(4) + 7;
        let width = height / 4 + random.next_int(2);
        if width > 1 && random.next_int(60) == 0 {
            origin_y += 10 + random.next_int(30);
        }

        let mut placed = false;
        for y_offset in 0..height {
            let scale = (1.0 - y_offset as f64 / height as f64) * width as f64;
            let radius = scale.ceil() as i32;

            for dx in -radius..=radius {
                let x_distance = dx.abs() as f64 - 0.25;
                for dz in -radius..=radius {
                    let z_distance = dz.abs() as f64 - 0.25;
                    let is_center = dx == 0 && dz == 0;
                    let is_edge = dx == -radius || dx == radius || dz == -radius || dz == radius;
                    if (is_center || x_distance * x_distance + z_distance * z_distance <= scale * scale)
                        && (!is_edge || random.next_float() <= 0.75)
                    {
                        let x = world_x + dx;
                        let z = world_z + dz;
                        if self.try_place(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            x,
                            origin_y + y_offset,
                            z,
                            settings.min_y,
                        ) {
                            placed = true;
                        }

                        if y_offset != 0
                            && radius > 1
                            && self.try_place(
                                chunk,
                                chunk_min_x,
                                chunk_min_z,
                                x,
                                origin_y - y_offset,
                                z,
                                settings.min_y,
                            )
                        {
                            placed = true;
                        }
                    }
                }
            }
        }

        let pillar_radius = (width - 1).clamp(0, 1);
        for dx in -pillar_radius..=pillar_radius {
            for dz in -pillar_radius..=pillar_radius {
                let mut y = origin_y - 1;
                let mut run_length = if dx.abs() == 1 && dz.abs() == 1 {
                    random.next_int(5)
                } else {
                    50
                };

                while y > 50 {
                    let Some(layer) =
                        layer_at_world(chunk, chunk_min_x, chunk_min_z, world_x + dx, y, world_z + dz, settings.min_y)
                    else {
                        break;
                    };
                    if !layer.is_air
                        && !is_ice_spike_replaceable_layer(layer)
                        && !layer.is(self.block.block.as_ref())
                    {
                        break;
                    }

                    if self.try_place(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x + dx,
                        y,
                        world_z + dz,
                        settings.min_y,
                    ) {
                        placed = true;
                    }
                    y -= 1;
                    run_length -= 1;
                    if run_length <= 0 {
                        y -= random.next_int(5) + 1;
                        run_length = random.next_int(5);
                    }
                }
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        let mut origin_y = world_y;
        origin_y += random.next_int(4);
        let height = random.next_int(4) + 7;
        let width = height / 4 + random.next_int(2);
        if width > 1 && random.next_int(60) == 0 {
            origin_y += 10 + random.next_int(30);
        }

        let mut placed = false;
        for y_offset in 0..height {
            let scale = (1.0 - y_offset as f64 / height as f64) * width as f64;
            let radius = scale.ceil() as i32;

            for dx in -radius..=radius {
                let x_distance = dx.abs() as f64 - 0.25;
                for dz in -radius..=radius {
                    let z_distance = dz.abs() as f64 - 0.25;
                    let is_center = dx == 0 && dz == 0;
                    let is_edge = dx == -radius || dx == radius || dz == -radius || dz == radius;
                    if (is_center
                        || x_distance * x_distance + z_distance * z_distance <= scale * scale)
                        && (!is_edge || random.next_float() <= 0.75)
                    {
                        let x = world_x + dx;
                        let z = world_z + dz;
                        if self.try_place_in_context(
                            source_chunk,
                            source_min_x,
                            source_min_z,
                            target_chunk,
                            target_min_x,
                            target_min_z,
                            x,
                            origin_y + y_offset,
                            z,
                            settings.min_y,
                        ) {
                            placed = true;
                        }

                        if y_offset != 0
                            && radius > 1
                            && self.try_place_in_context(
                                source_chunk,
                                source_min_x,
                                source_min_z,
                                target_chunk,
                                target_min_x,
                                target_min_z,
                                x,
                                origin_y - y_offset,
                                z,
                                settings.min_y,
                            )
                        {
                            placed = true;
                        }
                    }
                }
            }
        }

        let pillar_radius = (width - 1).clamp(0, 1);
        for dx in -pillar_radius..=pillar_radius {
            for dz in -pillar_radius..=pillar_radius {
                let mut y = origin_y - 1;
                let mut run_length = if dx.abs() == 1 && dz.abs() == 1 {
                    random.next_int(5)
                } else {
                    50
                };

                while y > 50 {
                    let x = world_x + dx;
                    let z = world_z + dz;
                    let Some(layer) = context_layer(
                        source_chunk,
                        source_min_x,
                        source_min_z,
                        target_chunk,
                        target_min_x,
                        target_min_z,
                        x,
                        y,
                        z,
                        settings.min_y,
                    ) else {
                        break;
                    };
                    if !layer.is_air
                        && !is_ice_spike_replaceable_layer(layer)
                        && !layer.is(self.block.block.as_ref())
                    {
                        break;
                    }

                    if self.try_place_in_context(
                        source_chunk,
                        source_min_x,
                        source_min_z,
                        target_chunk,
                        target_min_x,
                        target_min_z,
                        x,
                        y,
                        z,
                        settings.min_y,
                    ) {
                        placed = true;
                    }
                    y -= 1;
                    run_length -= 1;
                    if run_length <= 0 {
                        y -= random.next_int(5) + 1;
                        run_length = random.next_int(5);
                    }
                }
            }
        }

        placed
    }

    fn try_place(
        &self,
        chunk: &mut NoiseChunkBlocks,
        chunk_min_x: i32,
        chunk_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        let Some(current) =
            layer_at_world(chunk, chunk_min_x, chunk_min_z, world_x, world_y, world_z, min_y)
        else {
            return false;
        };
        if !current.is_air && !is_ice_spike_replaceable_layer(current) {
            return false;
        }
        set_surface_block_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            self.block.clone(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_place_in_context(
        &self,
        source_chunk: &mut NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        let Some(current) = context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        ) else {
            return false;
        };
        if !current.is_air && !is_ice_spike_replaceable_layer(current) {
            return false;
        }
        set_surface_block_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            self.block.clone(),
        )
    }
}

#[derive(Debug, Clone)]
struct SurfaceDiskConfig {
    half_height: i32,
    radius: UniformInt,
    block: BlockLayer,
    target_blocks: &'static [&'static str],
    required_anchor: &'static str,
}

impl SurfaceDiskConfig {
    fn ice_patch() -> Self {
        Self {
            half_height: 1,
            radius: UniformInt { min: 2, max: 3 },
            block: BlockLayer::new("minecraft:packed_ice"),
            target_blocks: ICE_PATCH_DISK_TARGETS,
            required_anchor: "minecraft:snow_block",
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_origin_y(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> Option<i32> {
        layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
            .is_some_and(|layer| layer.is(self.required_anchor))
            .then_some(world_y)
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        center_x: i32,
        center_y: i32,
        center_z: i32,
    ) -> bool {
        let radius = self.radius.sample(random);
        let min_y = (center_y - self.half_height).max(settings.min_y);
        let max_y = (center_y + self.half_height).min(settings.min_y + settings.height - 1);
        let mut placed = false;

        for world_x in center_x - radius..=center_x + radius {
            let dx = world_x - center_x;
            for world_z in center_z - radius..=center_z + radius {
                let dz = world_z - center_z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                for world_y in (min_y..=max_y).rev() {
                    let Some(current) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if self.target_blocks.contains(&current.block.as_ref())
                        && set_surface_block_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            world_y,
                            world_z,
                            settings.min_y,
                            self.block.clone(),
                        )
                    {
                        placed = true;
                    }
                }
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        center_x: i32,
        center_y: i32,
        center_z: i32,
    ) -> bool {
        let radius = self.radius.sample(random);
        let min_y = (center_y - self.half_height).max(settings.min_y);
        let max_y = (center_y + self.half_height).min(settings.min_y + settings.height - 1);
        let mut placed = false;

        for world_x in center_x - radius..=center_x + radius {
            let dx = world_x - center_x;
            for world_z in center_z - radius..=center_z + radius {
                let dz = world_z - center_z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                for world_y in (min_y..=max_y).rev() {
                    let Some(current) = context_layer(
                        source_chunk,
                        source_min_x,
                        source_min_z,
                        target_chunk,
                        target_min_x,
                        target_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if self.target_blocks.contains(&current.block.as_ref())
                        && set_surface_block_in_context(
                            source_chunk,
                            source_min_x,
                            source_min_z,
                            target_chunk,
                            target_min_x,
                            target_min_z,
                            world_x,
                            world_y,
                            world_z,
                            settings.min_y,
                            self.block.clone(),
                        )
                    {
                        placed = true;
                    }
                }
            }
        }

        placed
    }
}

#[derive(Debug, Clone)]
struct IcebergSurfaceConfig {
    block: BlockLayer,
    snow_block: BlockLayer,
    water_block: BlockLayer,
    air_block: BlockLayer,
}

impl IcebergSurfaceConfig {
    fn packed() -> Self {
        Self::new("minecraft:packed_ice")
    }

    fn blue() -> Self {
        Self::new("minecraft:blue_ice")
    }

    fn new(block: &str) -> Self {
        Self {
            block: BlockLayer::new(block),
            snow_block: BlockLayer::new("minecraft:snow_block"),
            water_block: BlockLayer::new("minecraft:water"),
            air_block: BlockLayer::new("minecraft:air"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        _world_y: i32,
        world_z: i32,
    ) -> bool {
        let origin_y = settings.sea_level;
        let snow_on_top = random.next_double() > 0.7;
        let shape_angle = random.next_double() * 2.0 * std::f64::consts::PI;
        let shape_ellipse_a = 11 - random.next_int(5);
        let shape_ellipse_c = 3 + random.next_int(3);
        let is_ellipse = random.next_double() > 0.7;
        let mut above_height = if is_ellipse {
            random.next_int(6) + 6
        } else {
            random.next_int(15) + 3
        };
        if !is_ellipse && random.next_double() > 0.9 {
            above_height += random.next_int(19) + 7;
        }

        let under_height = (above_height + random.next_int(11)).min(18);
        let width = (above_height + random.next_int(7) - random.next_int(5)).min(11);
        let max_radius = if is_ellipse { shape_ellipse_a } else { 11 };
        let mut placed = false;

        for dx in -max_radius..max_radius {
            for dz in -max_radius..max_radius {
                for y_offset in 0..above_height {
                    let radius = if is_ellipse {
                        height_dependent_radius_ellipse(y_offset, above_height, width)
                    } else {
                        height_dependent_radius_round(random, y_offset, above_height, width)
                    };
                    if is_ellipse || dx < radius {
                        placed |= self.generate_iceberg_block(
                            settings,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            random,
                            world_x,
                            origin_y,
                            world_z,
                            above_height,
                            dx,
                            y_offset,
                            dz,
                            radius,
                            max_radius,
                            is_ellipse,
                            shape_ellipse_c,
                            shape_angle,
                            snow_on_top,
                        );
                    }
                }
            }
        }

        self.smooth(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            world_x,
            origin_y,
            world_z,
            width,
            above_height,
            is_ellipse,
            shape_ellipse_a,
        );

        for dx in -max_radius..max_radius {
            for dz in -max_radius..max_radius {
                for y_offset in (-under_height + 1..= -1).rev() {
                    let steep_radius =
                        height_dependent_radius_steep(random, -y_offset, under_height, width);
                    let current_a = if is_ellipse {
                        ceil_i32(
                            shape_ellipse_a as f64
                                * (1.0
                                    - (y_offset * y_offset) as f64
                                        / (under_height * 8) as f64),
                        )
                    } else {
                        max_radius
                    };
                    if dx < steep_radius {
                        placed |= self.generate_iceberg_block(
                            settings,
                            chunk_min_x,
                            chunk_min_z,
                            chunk,
                            random,
                            world_x,
                            origin_y,
                            world_z,
                            under_height,
                            dx,
                            y_offset,
                            dz,
                            steep_radius,
                            current_a,
                            is_ellipse,
                            shape_ellipse_c,
                            shape_angle,
                            snow_on_top,
                        );
                    }
                }
            }
        }

        let do_cut_out = if is_ellipse {
            random.next_double() > 0.1
        } else {
            random.next_double() > 0.7
        };
        if do_cut_out {
            self.generate_cut_out(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                random,
                world_x,
                origin_y,
                world_z,
                width,
                above_height,
                is_ellipse,
                shape_ellipse_a,
                shape_angle,
                shape_ellipse_c,
            );
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        _world_y: i32,
        world_z: i32,
    ) -> bool {
        let origin_y = settings.sea_level;
        let snow_on_top = random.next_double() > 0.7;
        let shape_angle = random.next_double() * 2.0 * std::f64::consts::PI;
        let shape_ellipse_a = 11 - random.next_int(5);
        let shape_ellipse_c = 3 + random.next_int(3);
        let is_ellipse = random.next_double() > 0.7;
        let mut above_height = if is_ellipse {
            random.next_int(6) + 6
        } else {
            random.next_int(15) + 3
        };
        if !is_ellipse && random.next_double() > 0.9 {
            above_height += random.next_int(19) + 7;
        }

        let under_height = (above_height + random.next_int(11)).min(18);
        let width = (above_height + random.next_int(7) - random.next_int(5)).min(11);
        let max_radius = if is_ellipse { shape_ellipse_a } else { 11 };
        let mut placed = false;

        for dx in -max_radius..max_radius {
            for dz in -max_radius..max_radius {
                for y_offset in 0..above_height {
                    let radius = if is_ellipse {
                        height_dependent_radius_ellipse(y_offset, above_height, width)
                    } else {
                        height_dependent_radius_round(random, y_offset, above_height, width)
                    };
                    if is_ellipse || dx < radius {
                        placed |= self.generate_iceberg_block_in_context(
                            settings,
                            source_min_x,
                            source_min_z,
                            target_min_x,
                            target_min_z,
                            source_chunk,
                            target_chunk,
                            random,
                            world_x,
                            origin_y,
                            world_z,
                            above_height,
                            dx,
                            y_offset,
                            dz,
                            radius,
                            max_radius,
                            is_ellipse,
                            shape_ellipse_c,
                            shape_angle,
                            snow_on_top,
                        );
                    }
                }
            }
        }

        self.smooth_in_context(
            settings,
            source_min_x,
            source_min_z,
            target_min_x,
            target_min_z,
            source_chunk,
            target_chunk,
            world_x,
            origin_y,
            world_z,
            width,
            above_height,
            is_ellipse,
            shape_ellipse_a,
        );

        for dx in -max_radius..max_radius {
            for dz in -max_radius..max_radius {
                for y_offset in (-under_height + 1..=-1).rev() {
                    let steep_radius =
                        height_dependent_radius_steep(random, -y_offset, under_height, width);
                    let current_a = if is_ellipse {
                        ceil_i32(
                            shape_ellipse_a as f64
                                * (1.0
                                    - (y_offset * y_offset) as f64
                                        / (under_height * 8) as f64),
                        )
                    } else {
                        max_radius
                    };
                    if dx < steep_radius {
                        placed |= self.generate_iceberg_block_in_context(
                            settings,
                            source_min_x,
                            source_min_z,
                            target_min_x,
                            target_min_z,
                            source_chunk,
                            target_chunk,
                            random,
                            world_x,
                            origin_y,
                            world_z,
                            under_height,
                            dx,
                            y_offset,
                            dz,
                            steep_radius,
                            current_a,
                            is_ellipse,
                            shape_ellipse_c,
                            shape_angle,
                            snow_on_top,
                        );
                    }
                }
            }
        }

        let do_cut_out = if is_ellipse {
            random.next_double() > 0.1
        } else {
            random.next_double() > 0.7
        };
        if do_cut_out {
            self.generate_cut_out_in_context(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                random,
                world_x,
                origin_y,
                world_z,
                width,
                above_height,
                is_ellipse,
                shape_ellipse_a,
                shape_angle,
                shape_ellipse_c,
            );
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_iceberg_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        height: i32,
        dx: i32,
        y_offset: i32,
        dz: i32,
        radius: i32,
        a: i32,
        is_ellipse: bool,
        shape_ellipse_c: i32,
        shape_angle: f64,
        snow_on_top: bool,
    ) -> bool {
        let signed_distance = if is_ellipse {
            signed_distance_ellipse(
                dx,
                dz,
                0,
                0,
                a,
                iceberg_ellipse_c(y_offset, height, shape_ellipse_c),
                shape_angle,
            )
        } else {
            signed_distance_circle(dx, dz, 0, 0, radius, random)
        };
        if signed_distance >= 0.0 {
            return false;
        }

        let compare = if is_ellipse {
            -0.5
        } else {
            -6.0 - random.next_int(3) as f64
        };
        if signed_distance > compare && random.next_double() > 0.9 {
            return false;
        }

        self.set_iceberg_block(
            settings,
            chunk_min_x,
            chunk_min_z,
            chunk,
            random,
            origin_x + dx,
            origin_y + y_offset,
            origin_z + dz,
            height - y_offset,
            height,
            is_ellipse,
            snow_on_top,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_iceberg_block_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        height: i32,
        dx: i32,
        y_offset: i32,
        dz: i32,
        radius: i32,
        a: i32,
        is_ellipse: bool,
        shape_ellipse_c: i32,
        shape_angle: f64,
        snow_on_top: bool,
    ) -> bool {
        let signed_distance = if is_ellipse {
            signed_distance_ellipse(
                dx,
                dz,
                0,
                0,
                a,
                iceberg_ellipse_c(y_offset, height, shape_ellipse_c),
                shape_angle,
            )
        } else {
            signed_distance_circle(dx, dz, 0, 0, radius, random)
        };
        if signed_distance >= 0.0 {
            return false;
        }

        let compare = if is_ellipse {
            -0.5
        } else {
            -6.0 - random.next_int(3) as f64
        };
        if signed_distance > compare && random.next_double() > 0.9 {
            return false;
        }

        self.set_iceberg_block_in_context(
            settings,
            source_min_x,
            source_min_z,
            target_min_x,
            target_min_z,
            source_chunk,
            target_chunk,
            random,
            origin_x + dx,
            origin_y + y_offset,
            origin_z + dz,
            height - y_offset,
            height,
            is_ellipse,
            snow_on_top,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn set_iceberg_block(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        height_diff: i32,
        height: i32,
        is_ellipse: bool,
        snow_on_top: bool,
    ) -> bool {
        let Some(current) = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) else {
            return false;
        };
        if !current.is_air
            && !current.is("minecraft:snow_block")
            && !current.is("minecraft:ice")
            && !current.is("minecraft:water")
        {
            return false;
        }

        let randomness = !is_ellipse || random.next_double() > 0.05;
        let divisor = if is_ellipse { 3 } else { 2 };
        let max_snow_height =
            random.next_int((height / divisor).max(1)) as f64 + height as f64 * 0.6;
        let block = if snow_on_top
            && !current.is("minecraft:water")
            && height_diff as f64 <= max_snow_height
            && randomness
        {
            self.snow_block.clone()
        } else {
            self.block.clone()
        };

        set_surface_block_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
            block,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn set_iceberg_block_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        height_diff: i32,
        height: i32,
        is_ellipse: bool,
        snow_on_top: bool,
    ) -> bool {
        let Some(current) = context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        ) else {
            return false;
        };
        if !current.is_air
            && !current.is("minecraft:snow_block")
            && !current.is("minecraft:ice")
            && !current.is("minecraft:water")
        {
            return false;
        }

        let randomness = !is_ellipse || random.next_double() > 0.05;
        let divisor = if is_ellipse { 3 } else { 2 };
        let max_snow_height =
            random.next_int((height / divisor).max(1)) as f64 + height as f64 * 0.6;
        let block = if snow_on_top
            && !current.is("minecraft:water")
            && height_diff as f64 <= max_snow_height
            && randomness
        {
            self.snow_block.clone()
        } else {
            self.block.clone()
        };

        set_surface_block_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
            block,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_cut_out(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        width: i32,
        height: i32,
        is_ellipse: bool,
        shape_ellipse_a: i32,
        shape_angle: f64,
        shape_ellipse_c: i32,
    ) {
        let sign_x = if random.next_bool() { -1 } else { 1 };
        let sign_z = if random.next_bool() { -1 } else { 1 };
        let mut cut_x = random.next_int((width / 2 - 2).max(1));
        if random.next_bool() {
            cut_x = width / 2 + 1 - random.next_int((width - width / 2 - 1).max(1));
        }
        let mut cut_z = random.next_int((width / 2 - 2).max(1));
        if random.next_bool() {
            cut_z = width / 2 + 1 - random.next_int((width - width / 2 - 1).max(1));
        }
        if is_ellipse {
            let offset = random.next_int((shape_ellipse_a - 5).max(1));
            cut_x = offset;
            cut_z = offset;
        }

        let local_x = sign_x * cut_x;
        let local_z = sign_z * cut_z;
        let angle = if is_ellipse {
            shape_angle + std::f64::consts::FRAC_PI_2
        } else {
            random.next_double() * 2.0 * std::f64::consts::PI
        };

        for y_offset in 0..height - 3 {
            let radius = height_dependent_radius_round(random, y_offset, height, width);
            self.carve(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                origin_x,
                origin_y,
                origin_z,
                radius,
                y_offset,
                false,
                angle,
                local_x,
                local_z,
                shape_ellipse_a,
                shape_ellipse_c,
            );
        }

        let min_under_y = -height + random.next_int(5) + 1;
        for y_offset in (min_under_y..= -1).rev() {
            let radius = height_dependent_radius_steep(random, -y_offset, height, width);
            self.carve(
                settings,
                chunk_min_x,
                chunk_min_z,
                chunk,
                origin_x,
                origin_y,
                origin_z,
                radius,
                y_offset,
                true,
                angle,
                local_x,
                local_z,
                shape_ellipse_a,
                shape_ellipse_c,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_cut_out_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        width: i32,
        height: i32,
        is_ellipse: bool,
        shape_ellipse_a: i32,
        shape_angle: f64,
        shape_ellipse_c: i32,
    ) {
        let sign_x = if random.next_bool() { -1 } else { 1 };
        let sign_z = if random.next_bool() { -1 } else { 1 };
        let mut cut_x = random.next_int((width / 2 - 2).max(1));
        if random.next_bool() {
            cut_x = width / 2 + 1 - random.next_int((width - width / 2 - 1).max(1));
        }
        let mut cut_z = random.next_int((width / 2 - 2).max(1));
        if random.next_bool() {
            cut_z = width / 2 + 1 - random.next_int((width - width / 2 - 1).max(1));
        }
        if is_ellipse {
            let offset = random.next_int((shape_ellipse_a - 5).max(1));
            cut_x = offset;
            cut_z = offset;
        }

        let local_x = sign_x * cut_x;
        let local_z = sign_z * cut_z;
        let angle = if is_ellipse {
            shape_angle + std::f64::consts::FRAC_PI_2
        } else {
            random.next_double() * 2.0 * std::f64::consts::PI
        };

        for y_offset in 0..height - 3 {
            let radius = height_dependent_radius_round(random, y_offset, height, width);
            self.carve_in_context(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                origin_x,
                origin_y,
                origin_z,
                radius,
                y_offset,
                false,
                angle,
                local_x,
                local_z,
                shape_ellipse_a,
                shape_ellipse_c,
            );
        }

        let min_under_y = -height + random.next_int(5) + 1;
        for y_offset in (min_under_y..=-1).rev() {
            let radius = height_dependent_radius_steep(random, -y_offset, height, width);
            self.carve_in_context(
                settings,
                source_min_x,
                source_min_z,
                target_min_x,
                target_min_z,
                source_chunk,
                target_chunk,
                origin_x,
                origin_y,
                origin_z,
                radius,
                y_offset,
                true,
                angle,
                local_x,
                local_z,
                shape_ellipse_a,
                shape_ellipse_c,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn carve(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        radius: i32,
        y_offset: i32,
        underwater: bool,
        angle: f64,
        local_origin_x: i32,
        local_origin_z: i32,
        shape_ellipse_a: i32,
        shape_ellipse_c: i32,
    ) {
        let a = radius + 1 + shape_ellipse_a / 3;
        let c = (radius - 3).min(3) + shape_ellipse_c / 2 - 1;
        for dx in -a..a {
            for dz in -a..a {
                if signed_distance_ellipse(
                    dx,
                    dz,
                    local_origin_x,
                    local_origin_z,
                    a,
                    c,
                    angle,
                ) >= 0.0
                {
                    continue;
                }
                let world_x = origin_x + dx;
                let world_y = origin_y + y_offset;
                let world_z = origin_z + dz;
                let Some(current) = layer_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                ) else {
                    continue;
                };
                if !is_iceberg_layer(current) {
                    continue;
                }
                let replacement = if underwater {
                    self.water_block.clone()
                } else {
                    self.air_block.clone()
                };
                set_surface_block_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                    replacement,
                );
                if !underwater {
                    self.remove_floating_snow(
                        settings,
                        chunk_min_x,
                        chunk_min_z,
                        chunk,
                        world_x,
                        world_y + 1,
                        world_z,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn carve_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        radius: i32,
        y_offset: i32,
        underwater: bool,
        angle: f64,
        local_origin_x: i32,
        local_origin_z: i32,
        shape_ellipse_a: i32,
        shape_ellipse_c: i32,
    ) {
        let a = radius + 1 + shape_ellipse_a / 3;
        let c = (radius - 3).min(3) + shape_ellipse_c / 2 - 1;
        for dx in -a..a {
            for dz in -a..a {
                if signed_distance_ellipse(
                    dx,
                    dz,
                    local_origin_x,
                    local_origin_z,
                    a,
                    c,
                    angle,
                ) >= 0.0
                {
                    continue;
                }
                let world_x = origin_x + dx;
                let world_y = origin_y + y_offset;
                let world_z = origin_z + dz;
                let Some(current) = context_layer(
                    source_chunk,
                    source_min_x,
                    source_min_z,
                    target_chunk,
                    target_min_x,
                    target_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                ) else {
                    continue;
                };
                if !is_iceberg_layer(current) {
                    continue;
                }
                let replacement = if underwater {
                    self.water_block.clone()
                } else {
                    self.air_block.clone()
                };
                set_surface_block_in_context(
                    source_chunk,
                    source_min_x,
                    source_min_z,
                    target_chunk,
                    target_min_x,
                    target_min_z,
                    world_x,
                    world_y,
                    world_z,
                    settings.min_y,
                    replacement,
                );
                if !underwater {
                    self.remove_floating_snow_in_context(
                        settings,
                        source_min_x,
                        source_min_z,
                        target_min_x,
                        target_min_z,
                        source_chunk,
                        target_chunk,
                        world_x,
                        world_y + 1,
                        world_z,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn remove_floating_snow(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        if layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
        .is_some_and(|layer| layer.is("minecraft:snow"))
        {
            set_surface_block_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
                self.air_block.clone(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn remove_floating_snow_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) {
        if context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        )
        .is_some_and(|layer| layer.is("minecraft:snow"))
        {
            set_surface_block_in_context(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
                self.air_block.clone(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn smooth(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        width: i32,
        height: i32,
        is_ellipse: bool,
        shape_ellipse_a: i32,
    ) {
        let radius = if is_ellipse {
            shape_ellipse_a
        } else {
            width / 2
        };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                for y_offset in 0..=height {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + y_offset;
                    let world_z = origin_z + dz;
                    let Some(current) = layer_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if !is_iceberg_layer(current) && !current.is("minecraft:snow") {
                        continue;
                    }
                    if is_air_at_world(
                        chunk,
                        chunk_min_x,
                        chunk_min_z,
                        world_x,
                        world_y - 1,
                        world_z,
                        settings.min_y,
                    ) {
                        set_surface_block_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            world_y,
                            world_z,
                            settings.min_y,
                            self.air_block.clone(),
                        );
                        set_surface_block_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            world_y + 1,
                            world_z,
                            settings.min_y,
                            self.air_block.clone(),
                        );
                        continue;
                    }
                    if !is_iceberg_layer(current) {
                        continue;
                    }
                    let exposed_sides = [
                        (world_x - 1, world_z),
                        (world_x + 1, world_z),
                        (world_x, world_z - 1),
                        (world_x, world_z + 1),
                    ]
                    .into_iter()
                    .filter(|(x, z)| {
                        !layer_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            *x,
                            world_y,
                            *z,
                            settings.min_y,
                        )
                        .is_some_and(is_iceberg_layer)
                    })
                    .count();
                    if exposed_sides >= 3 {
                        set_surface_block_at_world(
                            chunk,
                            chunk_min_x,
                            chunk_min_z,
                            world_x,
                            world_y,
                            world_z,
                            settings.min_y,
                            self.air_block.clone(),
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn smooth_in_context(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        width: i32,
        height: i32,
        is_ellipse: bool,
        shape_ellipse_a: i32,
    ) {
        let radius = if is_ellipse {
            shape_ellipse_a
        } else {
            width / 2
        };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                for y_offset in 0..=height {
                    let world_x = origin_x + dx;
                    let world_y = origin_y + y_offset;
                    let world_z = origin_z + dz;
                    let Some(current) = context_layer(
                        source_chunk,
                        source_min_x,
                        source_min_z,
                        target_chunk,
                        target_min_x,
                        target_min_z,
                        world_x,
                        world_y,
                        world_z,
                        settings.min_y,
                    ) else {
                        continue;
                    };
                    if !is_iceberg_layer(current) && !current.is("minecraft:snow") {
                        continue;
                    }
                    if context_layer(
                        source_chunk,
                        source_min_x,
                        source_min_z,
                        target_chunk,
                        target_min_x,
                        target_min_z,
                        world_x,
                        world_y - 1,
                        world_z,
                        settings.min_y,
                    )
                    .is_some_and(|layer| layer.is_air)
                    {
                        set_surface_block_in_context(
                            source_chunk,
                            source_min_x,
                            source_min_z,
                            target_chunk,
                            target_min_x,
                            target_min_z,
                            world_x,
                            world_y,
                            world_z,
                            settings.min_y,
                            self.air_block.clone(),
                        );
                        set_surface_block_in_context(
                            source_chunk,
                            source_min_x,
                            source_min_z,
                            target_chunk,
                            target_min_x,
                            target_min_z,
                            world_x,
                            world_y + 1,
                            world_z,
                            settings.min_y,
                            self.air_block.clone(),
                        );
                        continue;
                    }
                    if !is_iceberg_layer(current) {
                        continue;
                    }
                    let exposed_sides = [
                        (world_x - 1, world_z),
                        (world_x + 1, world_z),
                        (world_x, world_z - 1),
                        (world_x, world_z + 1),
                    ]
                    .into_iter()
                    .filter(|(x, z)| {
                        !context_layer(
                            source_chunk,
                            source_min_x,
                            source_min_z,
                            target_chunk,
                            target_min_x,
                            target_min_z,
                            *x,
                            world_y,
                            *z,
                            settings.min_y,
                        )
                        .is_some_and(is_iceberg_layer)
                    })
                    .count();
                    if exposed_sides >= 3 {
                        set_surface_block_in_context(
                            source_chunk,
                            source_min_x,
                            source_min_z,
                            target_chunk,
                            target_min_x,
                            target_min_z,
                            world_x,
                            world_y,
                            world_z,
                            settings.min_y,
                            self.air_block.clone(),
                        );
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct BlueIceSurfaceConfig {
    block: BlockLayer,
}

impl BlueIceSurfaceConfig {
    fn new() -> Self {
        Self {
            block: BlockLayer::new("minecraft:blue_ice"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        settings: &NoiseSettings,
        chunk_min_x: i32,
        chunk_min_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if world_y > settings.sea_level - 1
            || (!is_water_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
            ) && !is_water_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y - 1,
                world_z,
                settings.min_y,
            ))
            || !has_adjacent_packed_ice(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
            )
        {
            return false;
        }

        set_surface_block_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
            self.block.clone(),
        );
        let mut placed = true;

        for _ in 0..200 {
            let y_offset = random.next_int(5) - random.next_int(6);
            let mut xz_diff = 3;
            if y_offset < 2 {
                xz_diff += y_offset / 2;
            }
            if xz_diff < 1 {
                continue;
            }
            let place_x = world_x + random.next_int(xz_diff) - random.next_int(xz_diff);
            let place_y = world_y + y_offset;
            let place_z = world_z + random.next_int(xz_diff) - random.next_int(xz_diff);
            let Some(current) = layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                place_x,
                place_y,
                place_z,
                settings.min_y,
            ) else {
                continue;
            };
            if (current.is_air
                || current.is("minecraft:water")
                || current.is("minecraft:packed_ice")
                || current.is("minecraft:ice"))
                && has_adjacent_blue_ice(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    place_x,
                    place_y,
                    place_z,
                    settings.min_y,
                )
                && set_surface_block_at_world(
                    chunk,
                    chunk_min_x,
                    chunk_min_z,
                    place_x,
                    place_y,
                    place_z,
                    settings.min_y,
                    self.block.clone(),
                )
            {
                placed = true;
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn place_with_spillover(
        &self,
        settings: &NoiseSettings,
        source_min_x: i32,
        source_min_z: i32,
        target_min_x: i32,
        target_min_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> bool {
        if world_y > settings.sea_level - 1
            || !self.is_water_context(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
            )
            || !self.has_adjacent_packed_ice_context(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                world_x,
                world_y,
                world_z,
                settings.min_y,
            )
        {
            return false;
        }

        let mut placed = self.set_blue_ice_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            settings.min_y,
        );

        for _ in 0..200 {
            let y_offset = random.next_int(5) - random.next_int(6);
            let mut xz_diff = 3;
            if y_offset < 2 {
                xz_diff += y_offset / 2;
            }
            if xz_diff < 1 {
                continue;
            }
            let place_x = world_x + random.next_int(xz_diff) - random.next_int(xz_diff);
            let place_y = world_y + y_offset;
            let place_z = world_z + random.next_int(xz_diff) - random.next_int(xz_diff);

            let Some(current) = self.context_layer(
                source_chunk,
                source_min_x,
                source_min_z,
                target_chunk,
                target_min_x,
                target_min_z,
                place_x,
                place_y,
                place_z,
                settings.min_y,
            ) else {
                continue;
            };
            if (current.is_air
                || current.is("minecraft:water")
                || current.is("minecraft:packed_ice")
                || current.is("minecraft:ice"))
                && self.has_adjacent_blue_ice_context(
                    source_chunk,
                    source_min_x,
                    source_min_z,
                    target_chunk,
                    target_min_x,
                    target_min_z,
                    place_x,
                    place_y,
                    place_z,
                    settings.min_y,
                )
                && self.set_blue_ice_in_context(
                    source_chunk,
                    source_min_x,
                    source_min_z,
                    target_chunk,
                    target_min_x,
                    target_min_z,
                    place_x,
                    place_y,
                    place_z,
                    settings.min_y,
                )
            {
                placed = true;
            }
        }

        placed
    }

    #[allow(clippy::too_many_arguments)]
    fn is_water_context(
        &self,
        source_chunk: &NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        self.context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        )
        .is_some_and(is_water_layer)
            || self
                .context_layer(
                    source_chunk,
                    source_min_x,
                    source_min_z,
                    target_chunk,
                    target_min_x,
                    target_min_z,
                    world_x,
                    world_y - 1,
                    world_z,
                    min_y,
                )
                .is_some_and(is_water_layer)
    }

    #[allow(clippy::too_many_arguments)]
    fn has_adjacent_packed_ice_context(
        &self,
        source_chunk: &NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        self.has_adjacent_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            "minecraft:packed_ice",
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn has_adjacent_blue_ice_context(
        &self,
        source_chunk: &NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        self.has_adjacent_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            "minecraft:blue_ice",
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn has_adjacent_context(
        &self,
        source_chunk: &NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
        block: &str,
        include_down: bool,
    ) -> bool {
        all_directions().into_iter().any(|direction| {
            (include_down || direction.3 != "down")
                && self
                    .context_layer(
                        source_chunk,
                        source_min_x,
                        source_min_z,
                        target_chunk,
                        target_min_x,
                        target_min_z,
                        world_x + direction.0,
                        world_y + direction.1,
                        world_z + direction.2,
                        min_y,
                    )
                    .is_some_and(|layer| layer.is(block))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn context_layer<'a>(
        &self,
        source_chunk: &'a NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &'a NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> Option<&'a BlockLayer> {
        context_layer(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn set_blue_ice_in_context(
        &self,
        source_chunk: &mut NoiseChunkBlocks,
        source_min_x: i32,
        source_min_z: i32,
        target_chunk: &mut NoiseChunkBlocks,
        target_min_x: i32,
        target_min_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        min_y: i32,
    ) -> bool {
        set_surface_block_in_context(
            source_chunk,
            source_min_x,
            source_min_z,
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            self.block.clone(),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn set_surface_block_at_world(
    chunk: &mut NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
        block: BlockLayer,
) -> bool {
    let Some((local_x, local_z)) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z) else {
        return false;
    };
    if chunk.layer(local_x, world_y, local_z, min_y).is_none() {
        return false;
    }
    chunk.set_layer(local_x, world_y, local_z, min_y, block);
    true
}

#[allow(clippy::too_many_arguments)]
fn set_surface_block_in_context(
    source_chunk: &mut NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &mut NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
    block: BlockLayer,
) -> bool {
    if overlaps_chunk(world_x, world_z, source_min_x, source_min_z) {
        set_surface_block_at_world(
            source_chunk,
            source_min_x,
            source_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            block,
        )
    } else if overlaps_chunk(world_x, world_z, target_min_x, target_min_z) {
        set_surface_block_at_world(
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
            block,
        )
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn context_layer<'a>(
    source_chunk: &'a NoiseChunkBlocks,
    source_min_x: i32,
    source_min_z: i32,
    target_chunk: &'a NoiseChunkBlocks,
    target_min_x: i32,
    target_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> Option<&'a BlockLayer> {
    layer_at_world(
        source_chunk,
        source_min_x,
        source_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .or_else(|| {
        layer_at_world(
            target_chunk,
            target_min_x,
            target_min_z,
            world_x,
            world_y,
            world_z,
            min_y,
        )
    })
}

fn squared_distance(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> f64 {
    let dx = x0 - x1;
    let dy = y0 - y1;
    let dz = z0 - z1;
    (dx * dx + dy * dy + dz * dz) as f64
}

fn signed_distance_circle(
    x: i32,
    z: i32,
    origin_x: i32,
    origin_z: i32,
    radius: i32,
    random: &mut FeatureRandom,
) -> f64 {
    let offset = 10.0 * random.next_float().clamp(0.2, 0.8) as f64 / radius as f64;
    offset + ((x - origin_x).pow(2) + (z - origin_z).pow(2)) as f64 - (radius * radius) as f64
}

fn signed_distance_ellipse(
    x: i32,
    z: i32,
    origin_x: i32,
    origin_z: i32,
    a: i32,
    c: i32,
    angle: f64,
) -> f64 {
    if a == 0 || c == 0 {
        return 1.0;
    }
    let x = (x - origin_x) as f64;
    let z = (z - origin_z) as f64;
    let a = a as f64;
    let c = c as f64;
    ((x * angle.cos() - z * angle.sin()) / a).powi(2)
        + ((x * angle.sin() + z * angle.cos()) / c).powi(2)
        - 1.0
}

fn height_dependent_radius_round(
    random: &mut FeatureRandom,
    y_offset: i32,
    height: i32,
    width: i32,
) -> i32 {
    let k = 3.5 - random.next_float() as f64;
    let mut scale = (1.0 - (y_offset * y_offset) as f64 / (height as f64 * k)) * width as f64;
    if height > 15 + random.next_int(5) {
        let adjusted_y = if y_offset < 3 + random.next_int(6) {
            y_offset / 2
        } else {
            y_offset
        };
        scale = (1.0 - adjusted_y as f64 / (height as f64 * k * 0.4)) * width as f64;
    }
    ceil_i32(scale / 2.0)
}

fn height_dependent_radius_ellipse(y_offset: i32, height: i32, width: i32) -> i32 {
    ceil_i32((1.0 - (y_offset * y_offset) as f64 / height as f64) * width as f64 / 2.0)
}

fn height_dependent_radius_steep(
    random: &mut FeatureRandom,
    y_offset: i32,
    height: i32,
    width: i32,
) -> i32 {
    let k = 1.0 + random.next_float() as f64 / 2.0;
    ceil_i32((1.0 - y_offset as f64 / (height as f64 * k)) * width as f64 / 2.0)
}

fn iceberg_ellipse_c(y_offset: i32, height: i32, shape_ellipse_c: i32) -> i32 {
    if y_offset > 0 && height - y_offset <= 3 {
        shape_ellipse_c - (4 - (height - y_offset))
    } else {
        shape_ellipse_c
    }
}

fn ceil_i32(value: f64) -> i32 {
    value.ceil() as i32
}

fn is_iceberg_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:packed_ice" | "minecraft:snow_block" | "minecraft:blue_ice"
    )
}

#[allow(clippy::too_many_arguments)]
fn has_adjacent_packed_ice(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    all_directions().into_iter().any(|direction| {
        direction.3 != "down"
            && layer_at_world(
                chunk,
                chunk_min_x,
                chunk_min_z,
                world_x + direction.0,
                world_y + direction.1,
                world_z + direction.2,
                min_y,
            )
            .is_some_and(|layer| layer.is("minecraft:packed_ice"))
    })
}

#[allow(clippy::too_many_arguments)]
fn has_adjacent_blue_ice(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    all_directions().into_iter().any(|direction| {
        layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x + direction.0,
            world_y + direction.1,
            world_z + direction.2,
            min_y,
        )
        .is_some_and(|layer| layer.is("minecraft:blue_ice"))
    })
}
