#[derive(Debug, Clone)]
struct VanillaCarvers {
    seed: i64,
    cave: CaveCarver,
    cave_extra_underground: CaveCarver,
    canyon: CanyonCarver,
}

impl VanillaCarvers {
    fn new(seed: i64) -> Self {
        Self {
            seed,
            cave: CaveCarver::default_cave(),
            cave_extra_underground: CaveCarver::extra_underground(),
            canyon: CanyonCarver::default_overworld(),
        }
    }

    fn carve_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
    ) {
        let mut mask = CarvingMask::new(settings.height as usize);
        for source_dx in -CARVER_SOURCE_RANGE..=CARVER_SOURCE_RANGE {
            for source_dz in -CARVER_SOURCE_RANGE..=CARVER_SOURCE_RANGE {
                let source_x = chunk_x + source_dx;
                let source_z = chunk_z + source_dz;
                let mut random = JavaRandom::new(0);
                random.set_large_feature_seed(self.seed, source_x, source_z);

                if self.cave.is_start_chunk(&mut random) {
                    self.cave.carve(
                        settings,
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        preliminary_surfaces,
                        chunk,
                        &mut mask,
                        &mut random,
                    );
                }

                random.set_large_feature_seed(self.seed + 1, source_x, source_z);
                if self.cave_extra_underground.is_start_chunk(&mut random) {
                    self.cave_extra_underground.carve(
                        settings,
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        preliminary_surfaces,
                        chunk,
                        &mut mask,
                        &mut random,
                    );
                }

                random.set_large_feature_seed(self.seed + 2, source_x, source_z);
                if self.canyon.is_start_chunk(&mut random) {
                    self.canyon.carve(
                        settings,
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        preliminary_surfaces,
                        chunk,
                        &mut mask,
                        &mut random,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CaveCarver {
    probability: f32,
    min_y: HeightAnchor,
    max_y: HeightAnchor,
    y_scale: UniformFloat,
    lava_level: HeightAnchor,
    horizontal_radius_multiplier: UniformFloat,
    vertical_radius_multiplier: UniformFloat,
    floor_level: UniformFloat,
}

impl CaveCarver {
    fn default_cave() -> Self {
        Self {
            probability: 0.15,
            min_y: HeightAnchor::AboveBottom(8),
            max_y: HeightAnchor::Absolute(180),
            y_scale: UniformFloat::new(0.1, 0.9),
            lava_level: HeightAnchor::AboveBottom(8),
            horizontal_radius_multiplier: UniformFloat::new(0.7, 1.4),
            vertical_radius_multiplier: UniformFloat::new(0.8, 1.3),
            floor_level: UniformFloat::new(-1.0, -0.4),
        }
    }

    fn extra_underground() -> Self {
        Self {
            probability: 0.07,
            min_y: HeightAnchor::AboveBottom(8),
            max_y: HeightAnchor::Absolute(47),
            y_scale: UniformFloat::new(0.1, 0.9),
            lava_level: HeightAnchor::AboveBottom(8),
            horizontal_radius_multiplier: UniformFloat::new(0.7, 1.4),
            vertical_radius_multiplier: UniformFloat::new(0.8, 1.3),
            floor_level: UniformFloat::new(-1.0, -0.4),
        }
    }

    fn is_start_chunk(&self, random: &mut JavaRandom) -> bool {
        random.next_float() <= self.probability
    }

    #[allow(clippy::too_many_arguments)]
    fn carve(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        source_x: i32,
        source_z: i32,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
        random: &mut JavaRandom,
    ) {
        let max_distance = (CARVER_RANGE * 2 - 1) * 16;
        let cave_bound = random.next_int(self.cave_bound()) + 1;
        let cave_bound = random.next_int(cave_bound) + 1;
        let cave_count = random.next_int(cave_bound);

        for _ in 0..cave_count {
            let x = source_x * 16 + random.next_int(16);
            let y = self.sample_y(random, settings) as f64;
            let z = source_z * 16 + random.next_int(16);
            let horizontal_radius_multiplier =
                self.horizontal_radius_multiplier.sample(random) as f64;
            let vertical_radius_multiplier = self.vertical_radius_multiplier.sample(random) as f64;
            let floor_level = self.floor_level.sample(random) as f64;
            let mut tunnels = 1;

            if random.next_int(4) == 0 {
                let y_scale = self.y_scale.sample(random) as f64;
                let thickness = 1.0 + random.next_float() * 6.0;
                self.create_room(
                    settings,
                    chunk_x,
                    chunk_z,
                    x as f64,
                    y,
                    z as f64,
                    thickness,
                    y_scale,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
                tunnels += random.next_int(4);
            }

            for _ in 0..tunnels {
                let horizontal_rotation = random.next_float() * std::f32::consts::TAU;
                let vertical_rotation = (random.next_float() - 0.5) / 4.0;
                let thickness = self.thickness(random);
                let distance = max_distance - random.next_int(max_distance / 4);
                self.create_tunnel(
                    settings,
                    chunk_x,
                    chunk_z,
                    random.next_long(),
                    x as f64,
                    y,
                    z as f64,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    thickness,
                    horizontal_rotation,
                    vertical_rotation,
                    0,
                    distance,
                    1.0,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
            }
        }
    }

    fn cave_bound(&self) -> i32 {
        15
    }

    fn thickness(&self, random: &mut JavaRandom) -> f32 {
        let mut thickness = random.next_float() * 2.0 + random.next_float();
        if random.next_int(10) == 0 {
            thickness *= random.next_float() * random.next_float() * 3.0 + 1.0;
        }
        thickness
    }

    fn sample_y(&self, random: &mut JavaRandom, settings: &NoiseSettings) -> i32 {
        let min = self.min_y.resolve(settings);
        let max = self.max_y.resolve(settings);
        if min > max {
            min
        } else {
            min + random.next_int(max - min + 1)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_room(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        x: f64,
        y: f64,
        z: f64,
        thickness: f32,
        y_scale: f64,
        floor_level: f64,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
    ) {
        let horizontal_radius = 1.5 + (std::f32::consts::FRAC_PI_2).sin() as f64 * thickness as f64;
        let vertical_radius = horizontal_radius * y_scale;
        carve_ellipsoid(
            settings,
            chunk_x,
            chunk_z,
            x + 1.0,
            y,
            z,
            horizontal_radius,
            vertical_radius,
            self.lava_level.resolve(settings),
            preliminary_surfaces,
            chunk,
            mask,
            |_, xd, yd, zd, _| cave_should_skip(xd, yd, zd, floor_level),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn create_tunnel(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        horizontal_radius_multiplier: f64,
        vertical_radius_multiplier: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        distance: i32,
        y_scale: f64,
        floor_level: f64,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
    ) {
        if distance <= 0 {
            return;
        }

        let mut random = JavaRandom::new(tunnel_seed);
        let split_point = random.next_int(distance / 2) + distance / 4;
        let steep = random.next_int(6) == 0;
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;

        for current_step in step..distance {
            let horizontal_radius = 1.5
                + (std::f32::consts::PI * current_step as f32 / distance as f32).sin() as f64
                    * thickness as f64;
            let vertical_radius = horizontal_radius * y_scale;
            let cos_x = vertical_rotation.cos();
            x += horizontal_rotation.cos() as f64 * cos_x as f64;
            y += vertical_rotation.sin() as f64;
            z += horizontal_rotation.sin() as f64 * cos_x as f64;
            vertical_rotation *= if steep { 0.92 } else { 0.7 };
            vertical_rotation += x_rota * 0.1;
            horizontal_rotation += y_rota * 0.1;
            x_rota *= 0.9;
            y_rota *= 0.75;
            x_rota += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            y_rota += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

            if current_step == split_point && thickness > 1.0 {
                self.create_tunnel(
                    settings,
                    chunk_x,
                    chunk_z,
                    random.next_long(),
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    random.next_float() * 0.5 + 0.5,
                    horizontal_rotation - std::f32::consts::FRAC_PI_2,
                    vertical_rotation / 3.0,
                    current_step,
                    distance,
                    1.0,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
                self.create_tunnel(
                    settings,
                    chunk_x,
                    chunk_z,
                    random.next_long(),
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    random.next_float() * 0.5 + 0.5,
                    horizontal_rotation + std::f32::consts::FRAC_PI_2,
                    vertical_rotation / 3.0,
                    current_step,
                    distance,
                    1.0,
                    floor_level,
                    preliminary_surfaces,
                    chunk,
                    mask,
                );
                return;
            }

            if random.next_int(4) != 0 {
                if !carver_can_reach(chunk_x, chunk_z, x, z, current_step, distance, thickness) {
                    return;
                }

                carve_ellipsoid(
                    settings,
                    chunk_x,
                    chunk_z,
                    x,
                    y,
                    z,
                    horizontal_radius * horizontal_radius_multiplier,
                    vertical_radius * vertical_radius_multiplier,
                    self.lava_level.resolve(settings),
                    preliminary_surfaces,
                    chunk,
                    mask,
                    |_, xd, yd, zd, _| cave_should_skip(xd, yd, zd, floor_level),
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CanyonCarver {
    probability: f32,
    min_y: HeightAnchor,
    max_y: HeightAnchor,
    y_scale: ConstantFloat,
    lava_level: HeightAnchor,
    vertical_rotation: UniformFloat,
    distance_factor: UniformFloat,
    thickness: TrapezoidFloat,
    width_smoothness: i32,
    horizontal_radius_factor: UniformFloat,
    vertical_radius_default_factor: f32,
    vertical_radius_center_factor: f32,
}

impl CanyonCarver {
    fn default_overworld() -> Self {
        Self {
            probability: 0.01,
            min_y: HeightAnchor::Absolute(10),
            max_y: HeightAnchor::Absolute(67),
            y_scale: ConstantFloat(3.0),
            lava_level: HeightAnchor::AboveBottom(8),
            vertical_rotation: UniformFloat::new(-0.125, 0.125),
            distance_factor: UniformFloat::new(0.75, 1.0),
            thickness: TrapezoidFloat {
                min: 0.0,
                max: 6.0,
                plateau: 2.0,
            },
            width_smoothness: 3,
            horizontal_radius_factor: UniformFloat::new(0.75, 1.0),
            vertical_radius_default_factor: 1.0,
            vertical_radius_center_factor: 0.0,
        }
    }

    fn is_start_chunk(&self, random: &mut JavaRandom) -> bool {
        random.next_float() <= self.probability
    }

    #[allow(clippy::too_many_arguments)]
    fn carve(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        source_x: i32,
        source_z: i32,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
        random: &mut JavaRandom,
    ) {
        let max_distance = (CARVER_RANGE * 2 - 1) * 16;
        let x = source_x * 16 + random.next_int(16);
        let y = self.sample_y(random, settings);
        let z = source_z * 16 + random.next_int(16);
        let horizontal_rotation = random.next_float() * std::f32::consts::TAU;
        let vertical_rotation = self.vertical_rotation.sample(random);
        let y_scale = self.y_scale.sample(random) as f64;
        let thickness = self.thickness.sample(random);
        let distance = (max_distance as f32 * self.distance_factor.sample(random)) as i32;

        self.do_carve(
            settings,
            chunk_x,
            chunk_z,
            random.next_long(),
            x as f64,
            y as f64,
            z as f64,
            thickness,
            horizontal_rotation,
            vertical_rotation,
            0,
            distance,
            y_scale,
            preliminary_surfaces,
            chunk,
            mask,
        );
    }

    fn sample_y(&self, random: &mut JavaRandom, settings: &NoiseSettings) -> i32 {
        let min = self.min_y.resolve(settings);
        let max = self.max_y.resolve(settings);
        if min > max {
            min
        } else {
            min + random.next_int(max - min + 1)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn do_carve(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        distance: i32,
        y_scale: f64,
        preliminary_surfaces: &[i32],
        chunk: &mut NoiseChunkBlocks,
        mask: &mut CarvingMask,
    ) {
        if distance <= 0 {
            return;
        }

        let mut random = JavaRandom::new(tunnel_seed);
        let width_factors = self.init_width_factors(settings, &mut random);
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;

        for current_step in step..distance {
            let mut horizontal_radius = 1.5
                + (current_step as f32 * std::f32::consts::PI / distance as f32).sin() as f64
                    * thickness as f64;
            let mut vertical_radius = horizontal_radius * y_scale;
            horizontal_radius *= self.horizontal_radius_factor.sample(&mut random) as f64;
            vertical_radius =
                self.update_vertical_radius(&mut random, vertical_radius, distance, current_step);

            let x_cos = vertical_rotation.cos();
            x += horizontal_rotation.cos() as f64 * x_cos as f64;
            y += vertical_rotation.sin() as f64;
            z += horizontal_rotation.sin() as f64 * x_cos as f64;
            vertical_rotation *= 0.7;
            vertical_rotation += x_rota * 0.05;
            horizontal_rotation += y_rota * 0.05;
            x_rota *= 0.8;
            y_rota *= 0.5;
            x_rota += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            y_rota += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

            if random.next_int(4) != 0 {
                if !carver_can_reach(chunk_x, chunk_z, x, z, current_step, distance, thickness) {
                    return;
                }

                carve_ellipsoid(
                    settings,
                    chunk_x,
                    chunk_z,
                    x,
                    y,
                    z,
                    horizontal_radius,
                    vertical_radius,
                    self.lava_level.resolve(settings),
                    preliminary_surfaces,
                    chunk,
                    mask,
                    |settings, xd, yd, zd, world_y| {
                        canyon_should_skip(settings, &width_factors, xd, yd, zd, world_y)
                    },
                );
            }
        }
    }

    fn init_width_factors(&self, settings: &NoiseSettings, random: &mut JavaRandom) -> Vec<f32> {
        let mut width_factors = vec![1.0; settings.height as usize];
        let mut width_factor = 1.0_f32;
        for (y_index, factor) in width_factors.iter_mut().enumerate() {
            if y_index == 0 || random.next_int(self.width_smoothness) == 0 {
                width_factor = 1.0 + random.next_float() * random.next_float();
            }
            *factor = width_factor * width_factor;
        }
        width_factors
    }

    fn update_vertical_radius(
        &self,
        random: &mut JavaRandom,
        vertical_radius: f64,
        distance: i32,
        current_step: i32,
    ) -> f64 {
        let vertical_multiplier = 1.0 - (0.5 - current_step as f32 / distance as f32).abs() * 2.0;
        let factor = self.vertical_radius_default_factor
            + self.vertical_radius_center_factor * vertical_multiplier;
        factor as f64 * vertical_radius * random_between(random, 0.75, 1.0) as f64
    }
}

#[allow(clippy::too_many_arguments)]
fn carve_ellipsoid<F>(
    settings: &NoiseSettings,
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    lava_level: i32,
    preliminary_surfaces: &[i32],
    chunk: &mut NoiseChunkBlocks,
    mask: &mut CarvingMask,
    should_skip: F,
) -> bool
where
    F: Fn(&NoiseSettings, f64, f64, f64, i32) -> bool,
{
    if horizontal_radius <= 0.0 || vertical_radius <= 0.0 {
        return false;
    }

    let center_x = chunk_x * 16 + 8;
    let center_z = chunk_z * 16 + 8;
    let max_delta = 16.0 + horizontal_radius * 2.0;
    if (x - center_x as f64).abs() > max_delta || (z - center_z as f64).abs() > max_delta {
        return false;
    }

    let chunk_min_x = chunk_x * 16;
    let chunk_min_z = chunk_z * 16;
    let min_x = ((x - horizontal_radius).floor() as i32 - chunk_min_x - 1).max(0);
    let max_x = ((x + horizontal_radius).floor() as i32 - chunk_min_x).min(15);
    let min_y = ((y - vertical_radius).floor() as i32 - 1).max(settings.min_y + 1);
    let max_y =
        ((y + vertical_radius).floor() as i32 + 1).min(settings.min_y + settings.height - 1 - 7);
    let min_z = ((z - horizontal_radius).floor() as i32 - chunk_min_z - 1).max(0);
    let max_z = ((z + horizontal_radius).floor() as i32 - chunk_min_z).min(15);
    let mut carved = false;

    for local_x in min_x..=max_x {
        let world_x = chunk_min_x + local_x;
        let xd = (world_x as f64 + 0.5 - x) / horizontal_radius;

        for local_z in min_z..=max_z {
            let world_z = chunk_min_z + local_z;
            let zd = (world_z as f64 + 0.5 - z) / horizontal_radius;
            if xd * xd + zd * zd >= 1.0 {
                continue;
            }

            let mut has_grass = false;
            for world_y in (min_y + 1..=max_y).rev() {
                let yd = (world_y as f64 - 0.5 - y) / vertical_radius;
                if should_skip(settings, xd, yd, zd, world_y)
                    || mask.get(local_x as usize, world_y, local_z as usize, settings.min_y)
                {
                    continue;
                }

                mask.set(local_x as usize, world_y, local_z as usize, settings.min_y);
                if carve_block(
                    settings,
                    chunk,
                    preliminary_surfaces,
                    world_x,
                    local_x as usize,
                    world_y,
                    world_z,
                    local_z as usize,
                    lava_level,
                    &mut has_grass,
                ) {
                    carved = true;
                }
            }
        }
    }

    carved
}

fn carve_block(
    settings: &NoiseSettings,
    chunk: &mut NoiseChunkBlocks,
    preliminary_surfaces: &[i32],
    world_x: i32,
    local_x: usize,
    world_y: i32,
    world_z: i32,
    local_z: usize,
    lava_level: i32,
    has_grass: &mut bool,
) -> bool {
    let Some(current) = chunk.layer(local_x, world_y, local_z, settings.min_y) else {
        return false;
    };
    if current.is("minecraft:grass_block") || current.is("minecraft:mycelium") {
        *has_grass = true;
    }
    if !is_overworld_carver_replaceable(current) {
        return false;
    }

    let surface_water_reachable = world_y < settings.sea_level
        && (chunk.surface_water_reaches(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            settings.sea_level,
        ) || chunk.surface_water_reaches_above(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            settings.sea_level,
        ) || chunk.adjacent_surface_water_reaches(
            local_x,
            world_y,
            local_z,
            settings.min_y,
            settings.sea_level,
        ));
    let Some(layer) = carve_layer(
        settings,
        world_x,
        world_y,
        world_z,
        preliminary_surfaces[local_z * 16 + local_x],
        lava_level,
        surface_water_reachable,
    ) else {
        return false;
    };
    chunk.set_layer(local_x, world_y, local_z, settings.min_y, layer.clone());

    if *has_grass
        && world_y > settings.min_y
        && chunk
            .layer(local_x, world_y - 1, local_z, settings.min_y)
            .is_some_and(|below| below.is("minecraft:dirt"))
    {
        chunk.set_layer(
            local_x,
            world_y - 1,
            local_z,
            settings.min_y,
            settings.surface_block.clone(),
        );
    }

    true
}

fn carve_layer(
    settings: &NoiseSettings,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    preliminary_surface: i32,
    lava_level: i32,
    surface_water_reachable: bool,
) -> Option<BlockLayer> {
    if world_y <= lava_level {
        Some(settings.lava_block.clone())
    } else if surface_water_reachable {
        Some(settings.default_fluid.clone())
    } else {
        match settings
            .aquifer
            .substance_at(world_x, world_y, world_z, 0.0, preliminary_surface)
        {
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Water) => {
                Some(settings.default_fluid.clone())
            }
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Lava) => {
                Some(settings.lava_block.clone())
            }
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Air) => {
                Some(settings.air_layer())
            }
            vanilla_noise::AquiferSubstance::DefaultBlock => None,
        }
    }
}

fn is_overworld_carver_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:stone"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:tuff"
            | "minecraft:deepslate"
            | "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
            | "minecraft:mud"
            | "minecraft:muddy_mangrove_roots"
            | "minecraft:moss_block"
            | "minecraft:pale_moss_block"
            | "minecraft:grass_block"
            | "minecraft:podzol"
            | "minecraft:mycelium"
            | "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:suspicious_sand"
            | "minecraft:terracotta"
            | "minecraft:white_terracotta"
            | "minecraft:orange_terracotta"
            | "minecraft:magenta_terracotta"
            | "minecraft:light_blue_terracotta"
            | "minecraft:yellow_terracotta"
            | "minecraft:lime_terracotta"
            | "minecraft:pink_terracotta"
            | "minecraft:gray_terracotta"
            | "minecraft:light_gray_terracotta"
            | "minecraft:cyan_terracotta"
            | "minecraft:purple_terracotta"
            | "minecraft:blue_terracotta"
            | "minecraft:brown_terracotta"
            | "minecraft:green_terracotta"
            | "minecraft:red_terracotta"
            | "minecraft:black_terracotta"
            | "minecraft:iron_ore"
            | "minecraft:deepslate_iron_ore"
            | "minecraft:copper_ore"
            | "minecraft:deepslate_copper_ore"
            | "minecraft:snow"
            | "minecraft:snow_block"
            | "minecraft:powder_snow"
            | "minecraft:water"
            | "minecraft:gravel"
            | "minecraft:suspicious_gravel"
            | "minecraft:sandstone"
            | "minecraft:red_sandstone"
            | "minecraft:calcite"
            | "minecraft:packed_ice"
            | "minecraft:raw_iron_block"
            | "minecraft:raw_copper_block"
    )
}

fn cave_should_skip(xd: f64, yd: f64, zd: f64, floor_level: f64) -> bool {
    yd <= floor_level || xd * xd + yd * yd + zd * zd >= 1.0
}

fn canyon_should_skip(
    settings: &NoiseSettings,
    width_factors: &[f32],
    xd: f64,
    yd: f64,
    zd: f64,
    world_y: i32,
) -> bool {
    let y_index = usize::try_from(world_y - settings.min_y - 1).unwrap_or(0);
    let width_factor = width_factors
        .get(y_index.min(width_factors.len().saturating_sub(1)))
        .copied()
        .unwrap_or(1.0) as f64;
    (xd * xd + zd * zd) * width_factor + yd * yd / 6.0 >= 1.0
}

fn carver_can_reach(
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    z: f64,
    current_step: i32,
    total_steps: i32,
    thickness: f32,
) -> bool {
    let x_mid = chunk_x * 16 + 8;
    let z_mid = chunk_z * 16 + 8;
    let xd = x - x_mid as f64;
    let zd = z - z_mid as f64;
    let remaining = (total_steps - current_step) as f64;
    let radius = thickness as f64 + 2.0 + 16.0;
    xd * xd + zd * zd - remaining * remaining <= radius * radius
}

#[derive(Clone, Debug)]
struct CarvingMask {
    height: usize,
    values: Vec<bool>,
}

impl CarvingMask {
    fn new(height: usize) -> Self {
        Self {
            height,
            values: vec![false; 16 * height * 16],
        }
    }

    fn get(&self, x: usize, y: i32, z: usize, min_y: i32) -> bool {
        self.index(x, y, z, min_y)
            .and_then(|index| self.values.get(index))
            .copied()
            .unwrap_or(false)
    }

    fn set(&mut self, x: usize, y: i32, z: usize, min_y: i32) {
        if let Some(index) = self.index(x, y, z, min_y)
            && let Some(value) = self.values.get_mut(index)
        {
            *value = true;
        }
    }

    fn index(&self, x: usize, y: i32, z: usize, min_y: i32) -> Option<usize> {
        if x >= 16 || z >= 16 {
            return None;
        }
        let y = usize::try_from(y - min_y).ok()?;
        (y < self.height).then_some((y * 16 + z) * 16 + x)
    }
}
