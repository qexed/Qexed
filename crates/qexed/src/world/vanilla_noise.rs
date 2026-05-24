const GOLDEN_RATIO_64: u64 = 0x9e37_79b9_7f4a_7c15;
const SILVER_RATIO_64: u64 = 0x6a09_e667_f3bc_c909;
const INPUT_WRAP: f64 = 33_554_432.0;
const NORMAL_NOISE_INPUT_FACTOR: f64 = 1.018_126_888_217_522_7;

const GRADIENT: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

const OFFSET_AMPLITUDES: [f64; 4] = [1.0, 1.0, 1.0, 0.0];
const CONTINENTALNESS_AMPLITUDES: [f64; 9] = [1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0];
const EROSION_AMPLITUDES: [f64; 5] = [1.0, 1.0, 0.0, 1.0, 1.0];
const RIDGE_AMPLITUDES: [f64; 6] = [1.0, 2.0, 1.0, 0.0, 0.0, 0.0];
const JAGGED_AMPLITUDES: [f64; 16] = [1.0; 16];
const NOODLE_AMPLITUDES: [f64; 1] = [1.0];
const NOODLE_THICKNESS_AMPLITUDES: [f64; 1] = [1.0];
const NOODLE_RIDGE_AMPLITUDES: [f64; 1] = [1.0];
const CAVE_LAYER_AMPLITUDES: [f64; 1] = [1.0];
const CAVE_CHEESE_AMPLITUDES: [f64; 9] = [0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0, 2.0, 0.0];
const CAVE_ENTRANCE_AMPLITUDES: [f64; 3] = [0.4, 0.5, 1.0];
const SPAGHETTI_3D_RARITY_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_3D_THICKNESS_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_3D_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_ROUGHNESS_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_ROUGHNESS_MODULATOR_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_2D_MODULATOR_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_2D_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_2D_ELEVATION_AMPLITUDES: [f64; 1] = [1.0];
const SPAGHETTI_2D_THICKNESS_AMPLITUDES: [f64; 1] = [1.0];
const PILLAR_AMPLITUDES: [f64; 2] = [1.0, 1.0];
const PILLAR_RARENESS_AMPLITUDES: [f64; 1] = [1.0];
const PILLAR_THICKNESS_AMPLITUDES: [f64; 1] = [1.0];
const OVERWORLD_OFFSET_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld/offset.json"
);
const OVERWORLD_FACTOR_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld/factor.json"
);
const OVERWORLD_JAGGEDNESS_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld/jaggedness.json"
);

#[derive(Clone, Debug)]
pub(crate) struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
}

impl BlendedNoise {
    pub(crate) fn overworld(seed: i64) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let factory = random.fork_positional();
        let mut terrain_random = factory.from_hash_of("minecraft:terrain");
        Self::new(&mut terrain_random, 0.25, 0.125, 80.0, 160.0, 8.0)
    }

    fn new(
        random: &mut XoroshiroRandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        Self {
            min_limit_noise: PerlinNoise::legacy_for_blended_noise(random, -15, 0),
            max_limit_noise: PerlinNoise::legacy_for_blended_noise(random, -15, 0),
            main_noise: PerlinNoise::legacy_for_blended_noise(random, -7, 0),
            xz_multiplier: 684.412 * xz_scale,
            y_multiplier: 684.412 * y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
        }
    }

    pub(crate) fn compute(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let limit_x = block_x as f64 * self.xz_multiplier;
        let limit_y = block_y as f64 * self.y_multiplier;
        let limit_z = block_z as f64 * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_y = limit_y / self.y_factor;
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;
        let mut blend_min = 0.0;
        let mut blend_max = 0.0;
        let mut main_noise_value = 0.0;
        let mut pow = 1.0;

        for i in 0..8 {
            if let Some(noise) = self.main_noise.get_octave_noise(i) {
                main_noise_value += noise.noise(
                    wrap(main_x * pow),
                    wrap(main_y * pow),
                    wrap(main_z * pow),
                    main_smear * pow,
                    main_y * pow,
                ) / pow;
            }
            pow /= 2.0;
        }

        let factor = (main_noise_value / 10.0 + 1.0) / 2.0;
        let is_max = factor >= 1.0;
        let is_min = factor <= 0.0;
        pow = 1.0;

        for i in 0..16 {
            let wx = wrap(limit_x * pow);
            let wy = wrap(limit_y * pow);
            let wz = wrap(limit_z * pow);
            let y_scale_pow = limit_smear * pow;

            if !is_max && let Some(noise) = self.min_limit_noise.get_octave_noise(i) {
                blend_min += noise.noise(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
            }

            if !is_min && let Some(noise) = self.max_limit_noise.get_octave_noise(i) {
                blend_max += noise.noise(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
            }

            pow /= 2.0;
        }

        clamped_lerp(factor, blend_min / 512.0, blend_max / 512.0) / 128.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OverworldClimateNoise {
    shift: NormalNoise,
    continentalness: NormalNoise,
    erosion: NormalNoise,
    ridge: NormalNoise,
}

impl OverworldClimateNoise {
    pub(crate) fn new(seed: i64) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let factory = random.fork_positional();
        Self {
            shift: NormalNoise::from_factory(&factory, "minecraft:offset", -3, &OFFSET_AMPLITUDES),
            continentalness: NormalNoise::from_factory(
                &factory,
                "minecraft:continentalness",
                -9,
                &CONTINENTALNESS_AMPLITUDES,
            ),
            erosion: NormalNoise::from_factory(
                &factory,
                "minecraft:erosion",
                -9,
                &EROSION_AMPLITUDES,
            ),
            ridge: NormalNoise::from_factory(&factory, "minecraft:ridge", -7, &RIDGE_AMPLITUDES),
        }
    }

    pub(crate) fn sample(&self, block_x: i32, block_z: i32) -> OverworldClimateSample {
        let x = block_x as f64;
        let z = block_z as f64;
        let shift_x = self.shift.get_value(x * 0.25, 0.0, z * 0.25) * 4.0;
        let shift_z = self.shift.get_value(z * 0.25, x * 0.25, 0.0) * 4.0;
        let sample_x = x * 0.25 + shift_x;
        let sample_z = z * 0.25 + shift_z;
        let ridges = self.ridge.get_value(sample_x, 0.0, sample_z);

        OverworldClimateSample {
            continentalness: self.continentalness.get_value(sample_x, 0.0, sample_z),
            erosion: self.erosion.get_value(sample_x, 0.0, sample_z),
            ridges,
            peaks_and_valleys: peaks_and_valleys(ridges),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverworldClimateSample {
    pub(crate) continentalness: f64,
    pub(crate) erosion: f64,
    pub(crate) ridges: f64,
    pub(crate) peaks_and_valleys: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct OverworldTerrainNoise {
    climate: OverworldClimateNoise,
    jagged: NormalNoise,
    caves: CaveNoise,
    noodle: NoodleNoise,
    offset: TerrainSpline,
    factor: TerrainSpline,
    jaggedness: TerrainSpline,
}

impl OverworldTerrainNoise {
    pub(crate) fn new(seed: i64) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let factory = random.fork_positional();
        Self {
            climate: OverworldClimateNoise::new(seed),
            jagged: NormalNoise::from_factory(
                &factory,
                "minecraft:jagged",
                -16,
                &JAGGED_AMPLITUDES,
            ),
            caves: CaveNoise::new(&factory),
            noodle: NoodleNoise::new(&factory),
            offset: terrain_spline_from_density_json(OVERWORLD_OFFSET_SPLINE),
            factor: terrain_spline_from_density_json(OVERWORLD_FACTOR_SPLINE),
            jaggedness: terrain_spline_from_density_json(OVERWORLD_JAGGEDNESS_SPLINE),
        }
    }

    pub(crate) fn profile(&self, block_x: i32, block_z: i32) -> OverworldTerrainProfile {
        let climate = self.climate.sample(block_x, block_z);
        OverworldTerrainProfile {
            offset: -0.503_750_026_226_043_7 + self.offset.apply(&climate),
            factor: self.factor.apply(&climate),
            jaggedness: self.jaggedness.apply(&climate),
        }
    }

    pub(crate) fn final_density(
        &self,
        profile: &OverworldTerrainProfile,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        base_3d_noise: f64,
    ) -> f64 {
        let sloped_cheese =
            self.sloped_cheese_density(profile, block_x, block_y, block_z, base_3d_noise);
        let entrances = self.caves.entrances(block_x, block_y, block_z);
        let caves = if sloped_cheese < 1.5625 {
            sloped_cheese.min(5.0 * entrances)
        } else {
            self.caves
                .underground_density(block_x, block_y, block_z, sloped_cheese, entrances)
        };
        let post_processed = post_process_density(slide_overworld(block_y, caves));
        post_processed.min(self.noodle.sample(block_x, block_y, block_z))
    }

    fn sloped_cheese_density(
        &self,
        profile: &OverworldTerrainProfile,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        base_3d_noise: f64,
    ) -> f64 {
        let depth = y_clamped_gradient(block_y, -64, 320, 1.5, -1.5) + profile.offset;
        let jagged = profile.jaggedness
            * half_negative(self.jagged.get_value(
                block_x as f64 * 1500.0,
                0.0,
                block_z as f64 * 1500.0,
            ));
        4.0 * quarter_negative((depth + jagged) * profile.factor) + base_3d_noise
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverworldTerrainProfile {
    offset: f64,
    factor: f64,
    jaggedness: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct OverworldSurfaceRules {
    bedrock_floor: PositionalRandomFactory,
    deepslate: PositionalRandomFactory,
}

impl OverworldSurfaceRules {
    pub(crate) fn new(seed: i64) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let root = random.fork_positional();
        let mut bedrock_floor = root.from_hash_of("minecraft:bedrock_floor");
        let mut deepslate = root.from_hash_of("minecraft:deepslate");
        Self {
            bedrock_floor: bedrock_floor.fork_positional(),
            deepslate: deepslate.fork_positional(),
        }
    }

    pub(crate) fn is_bedrock_floor(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        min_y: i32,
    ) -> bool {
        vertical_gradient(
            &self.bedrock_floor,
            block_x,
            block_y,
            block_z,
            min_y,
            min_y + 5,
        )
    }

    pub(crate) fn is_deepslate(&self, block_x: i32, block_y: i32, block_z: i32) -> bool {
        vertical_gradient(&self.deepslate, block_x, block_y, block_z, 0, 8)
    }
}

#[derive(Clone, Debug)]
struct CaveNoise {
    layer: NormalNoise,
    cheese: NormalNoise,
    entrance: NormalNoise,
    spaghetti_3d_rarity: NormalNoise,
    spaghetti_3d_thickness: NormalNoise,
    spaghetti_3d_1: NormalNoise,
    spaghetti_3d_2: NormalNoise,
    spaghetti_roughness: NormalNoise,
    spaghetti_roughness_modulator: NormalNoise,
    spaghetti_2d_modulator: NormalNoise,
    spaghetti_2d: NormalNoise,
    spaghetti_2d_elevation: NormalNoise,
    spaghetti_2d_thickness: NormalNoise,
    pillar: NormalNoise,
    pillar_rareness: NormalNoise,
    pillar_thickness: NormalNoise,
}

impl CaveNoise {
    fn new(factory: &PositionalRandomFactory) -> Self {
        Self {
            layer: NormalNoise::from_factory(
                factory,
                "minecraft:cave_layer",
                -8,
                &CAVE_LAYER_AMPLITUDES,
            ),
            cheese: NormalNoise::from_factory(
                factory,
                "minecraft:cave_cheese",
                -8,
                &CAVE_CHEESE_AMPLITUDES,
            ),
            entrance: NormalNoise::from_factory(
                factory,
                "minecraft:cave_entrance",
                -7,
                &CAVE_ENTRANCE_AMPLITUDES,
            ),
            spaghetti_3d_rarity: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_3d_rarity",
                -11,
                &SPAGHETTI_3D_RARITY_AMPLITUDES,
            ),
            spaghetti_3d_thickness: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_3d_thickness",
                -8,
                &SPAGHETTI_3D_THICKNESS_AMPLITUDES,
            ),
            spaghetti_3d_1: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_3d_1",
                -7,
                &SPAGHETTI_3D_AMPLITUDES,
            ),
            spaghetti_3d_2: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_3d_2",
                -7,
                &SPAGHETTI_3D_AMPLITUDES,
            ),
            spaghetti_roughness: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_roughness",
                -5,
                &SPAGHETTI_ROUGHNESS_AMPLITUDES,
            ),
            spaghetti_roughness_modulator: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_roughness_modulator",
                -8,
                &SPAGHETTI_ROUGHNESS_MODULATOR_AMPLITUDES,
            ),
            spaghetti_2d_modulator: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_2d_modulator",
                -11,
                &SPAGHETTI_2D_MODULATOR_AMPLITUDES,
            ),
            spaghetti_2d: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_2d",
                -7,
                &SPAGHETTI_2D_AMPLITUDES,
            ),
            spaghetti_2d_elevation: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_2d_elevation",
                -8,
                &SPAGHETTI_2D_ELEVATION_AMPLITUDES,
            ),
            spaghetti_2d_thickness: NormalNoise::from_factory(
                factory,
                "minecraft:spaghetti_2d_thickness",
                -11,
                &SPAGHETTI_2D_THICKNESS_AMPLITUDES,
            ),
            pillar: NormalNoise::from_factory(factory, "minecraft:pillar", -7, &PILLAR_AMPLITUDES),
            pillar_rareness: NormalNoise::from_factory(
                factory,
                "minecraft:pillar_rareness",
                -8,
                &PILLAR_RARENESS_AMPLITUDES,
            ),
            pillar_thickness: NormalNoise::from_factory(
                factory,
                "minecraft:pillar_thickness",
                -8,
                &PILLAR_THICKNESS_AMPLITUDES,
            ),
        }
    }

    fn underground_density(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        sloped_cheese: f64,
        entrances: f64,
    ) -> f64 {
        let base_cave_density = self.base_cave_density(block_x, block_y, block_z, sloped_cheese);
        let underground_subtractions = base_cave_density.min(entrances).min(
            self.spaghetti_2d(block_x, block_y, block_z)
                + self.spaghetti_roughness(block_x, block_y, block_z),
        );
        underground_subtractions.max(self.pillars(block_x, block_y, block_z))
    }

    fn base_cave_density(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        sloped_cheese: f64,
    ) -> f64 {
        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        let layer = 4.0 * self.layer.get_value(x, y * 8.0, z).powi(2);
        let cheese = (0.27 + self.cheese.get_value(x, y * (2.0 / 3.0), z)).clamp(-1.0, 1.0);
        let top_slide = (1.5 - 0.64 * sloped_cheese).clamp(0.0, 0.5);
        layer + cheese + top_slide
    }

    fn entrances(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        let rarity = self.spaghetti_3d_rarity.get_value(x * 2.0, y, z * 2.0);
        let thickness = mapped_unit_to(
            self.spaghetti_3d_thickness.get_value(x, y, z),
            -0.065,
            -0.088,
        );
        let cave_1 = weird_scaled_spaghetti_3d(&self.spaghetti_3d_1, rarity, x, y, z);
        let cave_2 = weird_scaled_spaghetti_3d(&self.spaghetti_3d_2, rarity, x, y, z);
        let spaghetti_3d = (cave_1.max(cave_2) + thickness).clamp(-1.0, 1.0);
        let roughness = self.spaghetti_roughness(block_x, block_y, block_z);
        let big_entrance = self.entrance.get_value(x * 0.75, y * 0.5, z * 0.75)
            + 0.37
            + y_clamped_gradient(block_y, -10, 30, 0.3, 0.0);

        big_entrance.min(roughness + spaghetti_3d)
    }

    fn spaghetti_roughness(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        let noise = self.spaghetti_roughness.get_value(x, y, z);
        let modulator = mapped_unit_to(
            self.spaghetti_roughness_modulator.get_value(x, y, z),
            0.0,
            -0.1,
        );
        modulator * (noise.abs() - 0.4)
    }

    fn spaghetti_2d(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        let rarity = self.spaghetti_2d_modulator.get_value(x * 2.0, y, z * 2.0);
        let cave = weird_scaled_spaghetti_2d(&self.spaghetti_2d, rarity, x, y, z);
        let elevation = mapped_unit_to(self.spaghetti_2d_elevation.get_value(x, 0.0, z), -8.0, 8.0);
        let thickness = mapped_unit_to(
            self.spaghetti_2d_thickness.get_value(x * 2.0, y, z * 2.0),
            -0.6,
            -1.3,
        );
        let sloped = (elevation + y_clamped_gradient(block_y, -64, 320, 8.0, -40.0)).abs();
        let layer_ridged = (sloped + thickness).powi(3);
        let cave_noise = cave + 0.083 * thickness;

        cave_noise.max(layer_ridged).clamp(-1.0, 1.0)
    }

    fn pillars(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        let pillar_noise = self.pillar.get_value(x * 25.0, y * 0.3, z * 25.0);
        let rareness = mapped_unit_to(self.pillar_rareness.get_value(x, y, z), 0.0, -2.0);
        let thickness = mapped_unit_to(self.pillar_thickness.get_value(x, y, z), 0.0, 1.1);
        let pillars = (2.0 * pillar_noise + rareness) * thickness.powi(3);

        if pillars < 0.03 {
            -1_000_000.0
        } else {
            pillars
        }
    }
}

#[derive(Clone, Debug)]
struct NoodleNoise {
    toggle: NormalNoise,
    thickness: NormalNoise,
    ridge_a: NormalNoise,
    ridge_b: NormalNoise,
}

impl NoodleNoise {
    fn new(factory: &PositionalRandomFactory) -> Self {
        Self {
            toggle: NormalNoise::from_factory(factory, "minecraft:noodle", -8, &NOODLE_AMPLITUDES),
            thickness: NormalNoise::from_factory(
                factory,
                "minecraft:noodle_thickness",
                -8,
                &NOODLE_THICKNESS_AMPLITUDES,
            ),
            ridge_a: NormalNoise::from_factory(
                factory,
                "minecraft:noodle_ridge_a",
                -7,
                &NOODLE_RIDGE_AMPLITUDES,
            ),
            ridge_b: NormalNoise::from_factory(
                factory,
                "minecraft:noodle_ridge_b",
                -7,
                &NOODLE_RIDGE_AMPLITUDES,
            ),
        }
    }

    fn sample(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        if !(-60..=320).contains(&block_y) {
            return 64.0;
        }

        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        if self.toggle.get_value(x, y, z) < 0.0 {
            return 64.0;
        }

        let thickness = -0.075 - 0.025 * self.thickness.get_value(x, y, z);
        let ridge_a = self
            .ridge_a
            .get_value(x * (8.0 / 3.0), y * (8.0 / 3.0), z * (8.0 / 3.0))
            .abs();
        let ridge_b = self
            .ridge_b
            .get_value(x * (8.0 / 3.0), y * (8.0 / 3.0), z * (8.0 / 3.0))
            .abs();
        thickness + 1.5 * ridge_a.max(ridge_b)
    }
}

#[derive(Clone, Debug)]
struct NormalNoise {
    value_factor: f64,
    first: PerlinNoise,
    second: PerlinNoise,
}

impl NormalNoise {
    fn from_factory(
        factory: &PositionalRandomFactory,
        name: &str,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let mut random = factory.from_hash_of(name);
        Self::new(&mut random, first_octave, amplitudes)
    }

    fn new(random: &mut XoroshiroRandomSource, first_octave: i32, amplitudes: &[f64]) -> Self {
        let first = PerlinNoise::create(random, first_octave, amplitudes.to_vec());
        let second = PerlinNoise::create(random, first_octave, amplitudes.to_vec());
        let mut min_octave = usize::MAX;
        let mut max_octave = 0;

        for (index, amplitude) in amplitudes.iter().enumerate() {
            if *amplitude != 0.0 {
                min_octave = min_octave.min(index);
                max_octave = max_octave.max(index);
            }
        }

        let octave_span = max_octave.saturating_sub(min_octave);
        let value_factor = (1.0 / 6.0) / expected_deviation(octave_span);
        Self {
            value_factor,
            first,
            second,
        }
    }

    fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let x2 = x * NORMAL_NOISE_INPUT_FACTOR;
        let y2 = y * NORMAL_NOISE_INPUT_FACTOR;
        let z2 = z * NORMAL_NOISE_INPUT_FACTOR;
        (self.first.get_value(x, y, z, 0.0, 0.0) + self.second.get_value(x2, y2, z2, 0.0, 0.0))
            * self.value_factor
    }
}

#[derive(Clone, Debug)]
struct TerrainSpline {
    coordinate: TerrainCoordinate,
    points: Vec<TerrainSplinePoint>,
}

impl TerrainSpline {
    fn apply(&self, climate: &OverworldClimateSample) -> f64 {
        let input = self.coordinate.apply(climate);
        let index = self.points.partition_point(|point| input >= point.location);
        if index == 0 {
            return self.points[0].extend(input, climate);
        }

        let start = index - 1;
        if start == self.points.len() - 1 {
            return self.points[start].extend(input, climate);
        }

        let left = &self.points[start];
        let right = &self.points[start + 1];
        let width = right.location - left.location;
        let t = (input - left.location) / width;
        let left_value = left.value.apply(climate);
        let right_value = right.value.apply(climate);
        let delta = right_value - left_value;
        let a = left.derivative * width - delta;
        let b = -right.derivative * width + delta;
        lerp(t, left_value, right_value) + t * (1.0 - t) * lerp(t, a, b)
    }
}

#[derive(Clone, Debug)]
struct TerrainSplinePoint {
    location: f64,
    value: TerrainSplineValue,
    derivative: f64,
}

impl TerrainSplinePoint {
    fn extend(&self, input: f64, climate: &OverworldClimateSample) -> f64 {
        let value = self.value.apply(climate);
        if self.derivative == 0.0 {
            value
        } else {
            value + self.derivative * (input - self.location)
        }
    }
}

#[derive(Clone, Debug)]
enum TerrainSplineValue {
    Constant(f64),
    Spline(Box<TerrainSpline>),
}

impl TerrainSplineValue {
    fn apply(&self, climate: &OverworldClimateSample) -> f64 {
        match self {
            Self::Constant(value) => *value,
            Self::Spline(spline) => spline.apply(climate),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TerrainCoordinate {
    Continents,
    Erosion,
    Ridges,
    RidgesFolded,
}

impl TerrainCoordinate {
    fn parse(value: &str) -> Self {
        match value {
            "minecraft:overworld/continents" => Self::Continents,
            "minecraft:overworld/erosion" => Self::Erosion,
            "minecraft:overworld/ridges" => Self::Ridges,
            "minecraft:overworld/ridges_folded" => Self::RidgesFolded,
            other => panic!("unsupported vanilla terrain spline coordinate: {other}"),
        }
    }

    fn apply(self, climate: &OverworldClimateSample) -> f64 {
        match self {
            Self::Continents => climate.continentalness,
            Self::Erosion => climate.erosion,
            Self::Ridges => climate.ridges,
            Self::RidgesFolded => climate.peaks_and_valleys,
        }
    }
}

#[derive(Clone, Debug)]
struct PerlinNoise {
    noise_levels: Vec<Option<ImprovedNoise>>,
    amplitudes: Vec<f64>,
    lowest_freq_input_factor: f64,
    lowest_freq_value_factor: f64,
}

impl PerlinNoise {
    fn legacy_for_blended_noise(
        random: &mut XoroshiroRandomSource,
        first_octave: i32,
        last_octave: i32,
    ) -> Self {
        let octaves = (last_octave - first_octave + 1) as usize;
        Self::legacy(random, first_octave, vec![1.0; octaves])
    }

    fn create(random: &mut XoroshiroRandomSource, first_octave: i32, amplitudes: Vec<f64>) -> Self {
        let octaves = amplitudes.len();
        let zero_octave_index = -first_octave;
        let mut noise_levels = vec![None; octaves];
        let positional = random.fork_positional();

        for (index, amplitude) in amplitudes.iter().enumerate() {
            if *amplitude != 0.0 {
                let octave = first_octave + index as i32;
                let mut octave_random = positional.from_hash_of(&format!("octave_{octave}"));
                noise_levels[index] = Some(ImprovedNoise::new(&mut octave_random));
            }
        }

        Self::from_levels(noise_levels, amplitudes, zero_octave_index)
    }

    fn legacy(random: &mut XoroshiroRandomSource, first_octave: i32, amplitudes: Vec<f64>) -> Self {
        let octaves = amplitudes.len();
        let zero_octave_index = -first_octave;
        let mut noise_levels = vec![None; octaves];
        let zero_octave = ImprovedNoise::new(random);

        if zero_octave_index >= 0 && (zero_octave_index as usize) < octaves {
            let index = zero_octave_index as usize;
            if amplitudes[index] != 0.0 {
                noise_levels[index] = Some(zero_octave);
            }
        }

        for i in (0..zero_octave_index).rev() {
            if (i as usize) < octaves {
                if amplitudes[i as usize] != 0.0 {
                    noise_levels[i as usize] = Some(ImprovedNoise::new(random));
                } else {
                    random.consume_count(262);
                }
            } else {
                random.consume_count(262);
            }
        }

        Self::from_levels(noise_levels, amplitudes, zero_octave_index)
    }

    fn from_levels(
        noise_levels: Vec<Option<ImprovedNoise>>,
        amplitudes: Vec<f64>,
        zero_octave_index: i32,
    ) -> Self {
        let octaves = amplitudes.len() as i32;
        let lowest_freq_input_factor = 2.0_f64.powi(-zero_octave_index);
        let lowest_freq_value_factor = 2.0_f64.powi(octaves - 1) / (2.0_f64.powi(octaves) - 1.0);
        Self {
            noise_levels,
            amplitudes,
            lowest_freq_input_factor,
            lowest_freq_value_factor,
        }
    }

    fn get_octave_noise(&self, octave: usize) -> Option<&ImprovedNoise> {
        self.noise_levels
            .get(self.noise_levels.len().checked_sub(1 + octave)?)
            .and_then(Option::as_ref)
    }

    fn get_value(&self, x: f64, y: f64, z: f64, y_scale: f64, y_fudge: f64) -> f64 {
        let mut value = 0.0;
        let mut factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;

        for (index, noise) in self.noise_levels.iter().enumerate() {
            if let Some(noise) = noise {
                let noise_value = noise.noise(
                    wrap(x * factor),
                    wrap(y * factor),
                    wrap(z * factor),
                    y_scale * factor,
                    y_fudge * factor,
                );
                value += self.amplitudes[index] * noise_value * value_factor;
            }

            factor *= 2.0;
            value_factor /= 2.0;
        }

        value
    }
}

#[derive(Clone, Debug)]
struct ImprovedNoise {
    p: [u8; 256],
    xo: f64,
    yo: f64,
    zo: f64,
}

impl ImprovedNoise {
    fn new(random: &mut XoroshiroRandomSource) -> Self {
        let xo = random.next_double() * 256.0;
        let yo = random.next_double() * 256.0;
        let zo = random.next_double() * 256.0;
        let mut p = [0_u8; 256];
        for (index, value) in p.iter_mut().enumerate() {
            *value = index as u8;
        }
        for i in 0..256 {
            let offset = random.next_int(256 - i);
            p.swap(i, i + offset);
        }
        Self { p, xo, yo, zo }
    }

    fn noise(&self, x: f64, y: f64, z: f64, y_scale: f64, y_fudge: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;
        let xf = x.floor() as i32;
        let yf = y.floor() as i32;
        let zf = z.floor() as i32;
        let xr = x - xf as f64;
        let yr = y - yf as f64;
        let zr = z - zf as f64;
        let yr_fudge = if y_scale != 0.0 {
            let fudge_limit = if y_fudge >= 0.0 && y_fudge < yr {
                y_fudge
            } else {
                yr
            };
            (fudge_limit / y_scale + 1.0e-7).floor() * y_scale
        } else {
            0.0
        };
        self.sample_and_lerp(xf, yf, zf, xr, yr - yr_fudge, zr, yr)
    }

    fn p(&self, index: i32) -> i32 {
        self.p[index.rem_euclid(256) as usize] as i32
    }

    fn sample_and_lerp(
        &self,
        x: i32,
        y: i32,
        z: i32,
        xr: f64,
        yr: f64,
        zr: f64,
        yr_original: f64,
    ) -> f64 {
        let x0 = self.p(x);
        let x1 = self.p(x + 1);
        let xy00 = self.p(x0 + y);
        let xy01 = self.p(x0 + y + 1);
        let xy10 = self.p(x1 + y);
        let xy11 = self.p(x1 + y + 1);
        let d000 = grad_dot(self.p(xy00 + z), xr, yr, zr);
        let d100 = grad_dot(self.p(xy10 + z), xr - 1.0, yr, zr);
        let d010 = grad_dot(self.p(xy01 + z), xr, yr - 1.0, zr);
        let d110 = grad_dot(self.p(xy11 + z), xr - 1.0, yr - 1.0, zr);
        let d001 = grad_dot(self.p(xy00 + z + 1), xr, yr, zr - 1.0);
        let d101 = grad_dot(self.p(xy10 + z + 1), xr - 1.0, yr, zr - 1.0);
        let d011 = grad_dot(self.p(xy01 + z + 1), xr, yr - 1.0, zr - 1.0);
        let d111 = grad_dot(self.p(xy11 + z + 1), xr - 1.0, yr - 1.0, zr - 1.0);
        lerp3(
            smoothstep(xr),
            smoothstep(yr_original),
            smoothstep(zr),
            d000,
            d100,
            d010,
            d110,
            d001,
            d101,
            d011,
            d111,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PositionalRandomFactory {
    seed_lo: u64,
    seed_hi: u64,
}

impl PositionalRandomFactory {
    fn from_hash_of(&self, name: &str) -> XoroshiroRandomSource {
        let seed = seed_from_hash_of(name).xor(self.seed_lo, self.seed_hi);
        XoroshiroRandomSource::from_seed128(seed)
    }

    fn at(&self, x: i32, y: i32, z: i32) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_raw_seed(position_seed(x, y, z) ^ self.seed_lo, self.seed_hi)
    }
}

#[derive(Clone, Debug)]
struct XoroshiroRandomSource {
    random: Xoroshiro128PlusPlus,
}

impl XoroshiroRandomSource {
    fn new(seed: i64) -> Self {
        Self::from_seed128(upgrade_seed_to_128bit(seed))
    }

    fn from_seed128(seed: Seed128bit) -> Self {
        Self {
            random: Xoroshiro128PlusPlus::new(seed.seed_lo, seed.seed_hi),
        }
    }

    fn from_raw_seed(seed_lo: u64, seed_hi: u64) -> Self {
        Self {
            random: Xoroshiro128PlusPlus::new(seed_lo, seed_hi),
        }
    }

    fn fork_positional(&mut self) -> PositionalRandomFactory {
        PositionalRandomFactory {
            seed_lo: self.next_long(),
            seed_hi: self.next_long(),
        }
    }

    fn next_long(&mut self) -> u64 {
        self.random.next_long()
    }

    fn next_int(&mut self, bound: usize) -> usize {
        assert!(bound > 0);
        let bound = bound as u64;
        let mut random_bits = self.next_int_raw() as u64;
        let mut multiplied = random_bits.wrapping_mul(bound);
        let mut fractional = multiplied & 0xffff_ffff;
        if fractional < bound {
            let threshold = (0_u32.wrapping_sub(bound as u32) as u64) % bound;
            while fractional < threshold {
                random_bits = self.next_int_raw() as u64;
                multiplied = random_bits.wrapping_mul(bound);
                fractional = multiplied & 0xffff_ffff;
            }
        }
        (multiplied >> 32) as usize
    }

    fn next_int_raw(&mut self) -> u32 {
        self.random.next_long() as u32
    }

    fn next_double(&mut self) -> f64 {
        (self.random.next_long() >> 11) as f64 * (1.110_223e-16_f32 as f64)
    }

    fn next_float(&mut self) -> f32 {
        (self.random.next_long() >> 40) as f32 * 5.960_464_5e-8_f32
    }

    fn consume_count(&mut self, rounds: usize) {
        for _ in 0..rounds {
            self.random.next_long();
        }
    }
}

#[derive(Clone, Debug)]
struct Xoroshiro128PlusPlus {
    seed_lo: u64,
    seed_hi: u64,
}

impl Xoroshiro128PlusPlus {
    fn new(seed_lo: u64, seed_hi: u64) -> Self {
        if seed_lo | seed_hi == 0 {
            Self {
                seed_lo: GOLDEN_RATIO_64,
                seed_hi: SILVER_RATIO_64,
            }
        } else {
            Self { seed_lo, seed_hi }
        }
    }

    fn next_long(&mut self) -> u64 {
        let s0 = self.seed_lo;
        let mut s1 = self.seed_hi;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.seed_lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.seed_hi = s1.rotate_left(28);
        result
    }
}

#[derive(Clone, Copy, Debug)]
struct Seed128bit {
    seed_lo: u64,
    seed_hi: u64,
}

impl Seed128bit {
    fn xor(self, lo: u64, hi: u64) -> Self {
        Self {
            seed_lo: self.seed_lo ^ lo,
            seed_hi: self.seed_hi ^ hi,
        }
    }

    fn mixed(self) -> Self {
        Self {
            seed_lo: mix_stafford13(self.seed_lo),
            seed_hi: mix_stafford13(self.seed_hi),
        }
    }
}

fn upgrade_seed_to_128bit(seed: i64) -> Seed128bit {
    let low = (seed as u64) ^ SILVER_RATIO_64;
    let high = low.wrapping_add(GOLDEN_RATIO_64);
    Seed128bit {
        seed_lo: low,
        seed_hi: high,
    }
    .mixed()
}

fn seed_from_hash_of(input: &str) -> Seed128bit {
    let digest = openssl::hash::hash(openssl::hash::MessageDigest::md5(), input.as_bytes())
        .expect("MD5 digest should be available");
    let bytes = digest.as_ref();
    let seed_lo = u64::from_be_bytes(bytes[0..8].try_into().expect("MD5 digest length"));
    let seed_hi = u64::from_be_bytes(bytes[8..16].try_into().expect("MD5 digest length"));
    Seed128bit { seed_lo, seed_hi }
}

fn position_seed(x: i32, y: i32, z: i32) -> u64 {
    let mut seed =
        (x.wrapping_mul(3_129_871) as i64) ^ (z as i64).wrapping_mul(116_129_781) ^ y as i64;
    seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42_317_861)
        .wrapping_add(seed.wrapping_mul(11));
    (seed >> 16) as u64
}

fn mix_stafford13(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul((-4658895280553007687_i64) as u64);
    value = (value ^ (value >> 27)).wrapping_mul((-7723592293110705685_i64) as u64);
    value ^ (value >> 31)
}

fn expected_deviation(octave_span: usize) -> f64 {
    0.1 * (1.0 + 1.0 / (octave_span as f64 + 1.0))
}

fn peaks_and_valleys(weirdness: f64) -> f64 {
    -((weirdness.abs() - 2.0 / 3.0).abs() - 1.0 / 3.0) * 3.0
}

fn terrain_spline_from_density_json(content: &str) -> TerrainSpline {
    let value: serde_json::Value =
        serde_json::from_str(content).expect("vanilla terrain density JSON must parse");
    let spline =
        find_spline_value(&value).expect("vanilla terrain density JSON must contain spline");
    parse_terrain_spline(spline)
}

fn find_spline_value(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|kind| kind == "minecraft:spline")
    {
        return value.get("spline");
    }

    match value {
        serde_json::Value::Array(values) => values.iter().find_map(find_spline_value),
        serde_json::Value::Object(values) => values.values().find_map(find_spline_value),
        _ => None,
    }
}

fn parse_terrain_spline(value: &serde_json::Value) -> TerrainSpline {
    let coordinate = value
        .get("coordinate")
        .and_then(serde_json::Value::as_str)
        .map(TerrainCoordinate::parse)
        .expect("vanilla terrain spline must have coordinate");
    let points = value
        .get("points")
        .and_then(serde_json::Value::as_array)
        .expect("vanilla terrain spline must have points")
        .iter()
        .map(parse_terrain_spline_point)
        .collect();
    TerrainSpline { coordinate, points }
}

fn parse_terrain_spline_point(value: &serde_json::Value) -> TerrainSplinePoint {
    TerrainSplinePoint {
        location: json_f64(value, "location"),
        value: parse_terrain_spline_value(
            value
                .get("value")
                .expect("vanilla terrain spline point must have value"),
        ),
        derivative: json_f64(value, "derivative"),
    }
}

fn parse_terrain_spline_value(value: &serde_json::Value) -> TerrainSplineValue {
    if let Some(value) = value.as_f64() {
        TerrainSplineValue::Constant(value)
    } else {
        TerrainSplineValue::Spline(Box::new(parse_terrain_spline(value)))
    }
}

fn json_f64(value: &serde_json::Value, field: &'static str) -> f64 {
    value
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("vanilla terrain spline point must have numeric {field}"))
}

fn y_clamped_gradient(y: i32, from_y: i32, to_y: i32, from_value: f64, to_value: f64) -> f64 {
    clamped_lerp(
        (y - from_y) as f64 / (to_y - from_y) as f64,
        from_value,
        to_value,
    )
}

fn half_negative(value: f64) -> f64 {
    if value > 0.0 { value } else { value * 0.5 }
}

fn quarter_negative(value: f64) -> f64 {
    if value > 0.0 { value } else { value * 0.25 }
}

fn vertical_gradient(
    factory: &PositionalRandomFactory,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    true_at_and_below: i32,
    false_at_and_above: i32,
) -> bool {
    if block_y <= true_at_and_below {
        return true;
    }

    if block_y >= false_at_and_above {
        return false;
    }

    let probability =
        (false_at_and_above - block_y) as f64 / (false_at_and_above - true_at_and_below) as f64;
    factory.at(block_x, block_y, block_z).next_float() < probability as f32
}

fn mapped_unit_to(value: f64, min: f64, max: f64) -> f64 {
    (min + max) * 0.5 + value * ((max - min) * 0.5)
}

fn weird_scaled_spaghetti_3d(
    noise: &NormalNoise,
    rarity_factor: f64,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let rarity = spaghetti_rarity_3d(rarity_factor);
    rarity * noise.get_value(x / rarity, y / rarity, z / rarity).abs()
}

fn spaghetti_rarity_3d(value: f64) -> f64 {
    if value < -0.5 {
        0.75
    } else if value < 0.0 {
        1.0
    } else if value < 0.5 {
        1.5
    } else {
        2.0
    }
}

fn weird_scaled_spaghetti_2d(
    noise: &NormalNoise,
    rarity_factor: f64,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let rarity = spaghetti_rarity_2d(rarity_factor);
    rarity * noise.get_value(x / rarity, y / rarity, z / rarity).abs()
}

fn spaghetti_rarity_2d(value: f64) -> f64 {
    if value < -0.75 {
        0.5
    } else if value < -0.5 {
        0.75
    } else if value < 0.5 {
        1.0
    } else if value < 0.75 {
        2.0
    } else {
        3.0
    }
}

fn post_process_density(value: f64) -> f64 {
    squeeze(value * 0.64)
}

fn slide_overworld(y: i32, density: f64) -> f64 {
    slide(y, density, -64, 384, 80, 64, -0.078_125, 0, 24, 0.117_187_5)
}

#[allow(clippy::too_many_arguments)]
fn slide(
    y: i32,
    density: f64,
    min_y: i32,
    height: i32,
    top_start_y: i32,
    top_end_y: i32,
    top_target: f64,
    bottom_start_y: i32,
    bottom_end_y: i32,
    bottom_target: f64,
) -> f64 {
    let top_factor = y_clamped_gradient(
        y,
        min_y + height - top_start_y,
        min_y + height - top_end_y,
        1.0,
        0.0,
    );
    let density = lerp(top_factor, top_target, density);
    let bottom_factor =
        y_clamped_gradient(y, min_y + bottom_start_y, min_y + bottom_end_y, 0.0, 1.0);
    lerp(bottom_factor, bottom_target, density)
}

fn squeeze(value: f64) -> f64 {
    let value = value.clamp(-1.0, 1.0);
    value / 2.0 - value * value * value / 24.0
}

fn grad_dot(hash: i32, x: f64, y: f64, z: f64) -> f64 {
    let gradient = GRADIENT[(hash & 15) as usize];
    gradient[0] as f64 * x + gradient[1] as f64 * y + gradient[2] as f64 * z
}

fn wrap(value: f64) -> f64 {
    value - (value / INPUT_WRAP + 0.5).floor() * INPUT_WRAP
}

fn smoothstep(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn lerp(delta: f64, min: f64, max: f64) -> f64 {
    min + delta * (max - min)
}

fn clamped_lerp(delta: f64, min: f64, max: f64) -> f64 {
    if delta < 0.0 {
        min
    } else if delta > 1.0 {
        max
    } else {
        lerp(delta, min, max)
    }
}

fn lerp2(delta1: f64, delta2: f64, x00: f64, x10: f64, x01: f64, x11: f64) -> f64 {
    lerp(delta2, lerp(delta1, x00, x10), lerp(delta1, x01, x11))
}

#[allow(clippy::too_many_arguments)]
fn lerp3(
    delta1: f64,
    delta2: f64,
    delta3: f64,
    x000: f64,
    x100: f64,
    x010: f64,
    x110: f64,
    x001: f64,
    x101: f64,
    x011: f64,
    x111: f64,
) -> f64 {
    lerp(
        delta3,
        lerp2(delta1, delta2, x000, x100, x010, x110),
        lerp2(delta1, delta2, x001, x101, x011, x111),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xoroshiro_seed_upgrade_is_stable() {
        let mut random = XoroshiroRandomSource::new(0);

        assert_eq!(random.next_long(), 0x2a2c_a488_f66f_517e);
        assert_eq!(random.next_long(), 0xccbc_22d7_2e97_c372);
    }

    #[test]
    fn position_seed_matches_vanilla_overflow_math() {
        assert_eq!(position_seed(1, 2, 3), (-33_674_130_277_896_i64) as u64);
        assert_eq!(
            position_seed(123, -17, -456),
            (-125_557_560_602_363_i64) as u64
        );
        assert_eq!(
            position_seed(-30_000_000, 7, 40_000_000),
            21_814_752_170_297
        );
    }

    #[test]
    fn blended_noise_is_deterministic() {
        let noise = BlendedNoise::overworld(0);

        assert_eq!(
            noise.compute(0, 64, 0).to_bits(),
            BlendedNoise::overworld(0).compute(0, 64, 0).to_bits()
        );
        assert_ne!(
            noise.compute(0, 64, 0).to_bits(),
            BlendedNoise::overworld(1).compute(0, 64, 0).to_bits()
        );
    }
}
