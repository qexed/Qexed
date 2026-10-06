#![allow(dead_code)]

use std::sync::OnceLock;

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
const TEMPERATURE_AMPLITUDES: [f64; 6] = [1.5, 0.0, 1.0, 0.0, 0.0, 0.0];
const VEGETATION_AMPLITUDES: [f64; 6] = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
const CONTINENTALNESS_AMPLITUDES: [f64; 9] = [1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0];
const EROSION_AMPLITUDES: [f64; 5] = [1.0, 1.0, 0.0, 1.0, 1.0];
const RIDGE_AMPLITUDES: [f64; 6] = [1.0, 2.0, 1.0, 0.0, 0.0, 0.0];
const TEMPERATURE_LARGE_FIRST_OCTAVE: i32 = -12;
const VEGETATION_LARGE_FIRST_OCTAVE: i32 = -10;
const CONTINENTALNESS_LARGE_FIRST_OCTAVE: i32 = -11;
const EROSION_LARGE_FIRST_OCTAVE: i32 = -11;
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
const AQUIFER_FLOODEDNESS_AMPLITUDES: [f64; 1] = [1.0];
const AQUIFER_SPREAD_AMPLITUDES: [f64; 1] = [1.0];
const AQUIFER_LAVA_AMPLITUDES: [f64; 1] = [1.0];
const AQUIFER_BARRIER_AMPLITUDES: [f64; 1] = [1.0];
const ORE_VEININESS_AMPLITUDES: [f64; 1] = [1.0];
const ORE_VEIN_RIDGE_AMPLITUDES: [f64; 1] = [1.0];
const ORE_GAP_AMPLITUDES: [f64; 1] = [1.0];
const SURFACE_AMPLITUDES: [f64; 3] = [1.0, 1.0, 1.0];
const SURFACE_SWAMP_AMPLITUDES: [f64; 1] = [1.0];
const CALCITE_AMPLITUDES: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
const GRAVEL_AMPLITUDES: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
const PACKED_ICE_AMPLITUDES: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
const ICE_AMPLITUDES: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
const POWDER_SNOW_AMPLITUDES: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
const CLAY_BANDS_OFFSET_AMPLITUDES: [f64; 1] = [1.0];
const OVERWORLD_OFFSET_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld/offset.json"
);
const OVERWORLD_FACTOR_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld/factor.json"
);
const OVERWORLD_JAGGEDNESS_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld/jaggedness.json"
);
const OVERWORLD_LARGE_OFFSET_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld_large_biomes/offset.json"
);
const OVERWORLD_LARGE_FACTOR_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld_large_biomes/factor.json"
);
const OVERWORLD_LARGE_JAGGEDNESS_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld_large_biomes/jaggedness.json"
);
const OVERWORLD_AMPLIFIED_OFFSET_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld_amplified/offset.json"
);
const OVERWORLD_AMPLIFIED_FACTOR_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld_amplified/factor.json"
);
const OVERWORLD_AMPLIFIED_JAGGEDNESS_SPLINE: &str = include_str!(
    "../../../../assets/decompiled_source/src/data/minecraft/worldgen/density_function/overworld_amplified/jaggedness.json"
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
        self.column_sampler(block_x, block_z).compute(block_y)
    }

    pub(crate) fn column_sampler(&self, block_x: i32, block_z: i32) -> BlendedNoiseColumn<'_> {
        let limit_x = block_x as f64 * self.xz_multiplier;
        let limit_z = block_z as f64 * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        let mut main_xz = [(0.0, 0.0); 8];
        let mut limit_xz = [(0.0, 0.0); 16];
        let mut pow = 1.0;
        for entry in &mut main_xz {
            *entry = (wrap(main_x * pow), wrap(main_z * pow));
            pow /= 2.0;
        }

        pow = 1.0;
        for entry in &mut limit_xz {
            *entry = (wrap(limit_x * pow), wrap(limit_z * pow));
            pow /= 2.0;
        }

        BlendedNoiseColumn {
            noise: self,
            main_xz,
            limit_xz,
            y_multiplier: self.y_multiplier,
            y_factor: self.y_factor,
            limit_smear,
            main_smear,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BlendedNoiseColumn<'a> {
    noise: &'a BlendedNoise,
    main_xz: [(f64, f64); 8],
    limit_xz: [(f64, f64); 16],
    y_multiplier: f64,
    y_factor: f64,
    limit_smear: f64,
    main_smear: f64,
}

impl BlendedNoiseColumn<'_> {
    pub(crate) fn compute(&self, block_y: i32) -> f64 {
        let limit_y = block_y as f64 * self.y_multiplier;
        let main_y = limit_y / self.y_factor;
        let mut blend_min = 0.0;
        let mut blend_max = 0.0;
        let mut main_noise_value = 0.0;
        let mut pow = 1.0;

        for i in 0..8 {
            if let Some(noise) = self.noise.main_noise.get_octave_noise(i) {
                let (main_x, main_z) = self.main_xz[i];
                main_noise_value += noise.noise(
                    main_x,
                    wrap(main_y * pow),
                    main_z,
                    self.main_smear * pow,
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
            let (wx, wz) = self.limit_xz[i];
            let wy = wrap(limit_y * pow);
            let y_scale_pow = self.limit_smear * pow;

            if !is_max && let Some(noise) = self.noise.min_limit_noise.get_octave_noise(i) {
                blend_min += noise.noise(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
            }

            if !is_min && let Some(noise) = self.noise.max_limit_noise.get_octave_noise(i) {
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
    temperature: NormalNoise,
    vegetation: NormalNoise,
    continentalness: NormalNoise,
    erosion: NormalNoise,
    ridge: NormalNoise,
}

impl OverworldClimateNoise {
    pub(crate) fn new(seed: i64) -> Self {
        Self::with_kind(seed, OverworldNoiseKind::Default)
    }

    pub(crate) fn large_biomes(seed: i64) -> Self {
        Self::with_kind(seed, OverworldNoiseKind::LargeBiomes)
    }

    fn with_kind(seed: i64, kind: OverworldNoiseKind) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let factory = random.fork_positional();
        Self {
            shift: NormalNoise::from_factory(&factory, "minecraft:offset", -3, &OFFSET_AMPLITUDES),
            temperature: NormalNoise::from_factory(
                &factory,
                kind.temperature_noise(),
                kind.temperature_first_octave(),
                &TEMPERATURE_AMPLITUDES,
            ),
            vegetation: NormalNoise::from_factory(
                &factory,
                kind.vegetation_noise(),
                kind.vegetation_first_octave(),
                &VEGETATION_AMPLITUDES,
            ),
            continentalness: NormalNoise::from_factory(
                &factory,
                kind.continentalness_noise(),
                kind.continentalness_first_octave(),
                &CONTINENTALNESS_AMPLITUDES,
            ),
            erosion: NormalNoise::from_factory(
                &factory,
                kind.erosion_noise(),
                kind.erosion_first_octave(),
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
            temperature: self.temperature.get_value(sample_x, 0.0, sample_z),
            vegetation: self.vegetation.get_value(sample_x, 0.0, sample_z),
            continentalness: self.continentalness.get_value(sample_x, 0.0, sample_z),
            erosion: self.erosion.get_value(sample_x, 0.0, sample_z),
            ridges,
            peaks_and_valleys: peaks_and_valleys(ridges),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverworldClimateSample {
    pub(crate) temperature: f64,
    pub(crate) vegetation: f64,
    pub(crate) continentalness: f64,
    pub(crate) erosion: f64,
    pub(crate) ridges: f64,
    pub(crate) peaks_and_valleys: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverworldNoiseKind {
    Default,
    LargeBiomes,
    Amplified,
}

impl OverworldNoiseKind {
    pub(crate) fn from_preset(preset: &str) -> Self {
        let name = preset
            .strip_prefix("minecraft:")
            .or_else(|| preset.strip_prefix(':'))
            .unwrap_or(preset);
        match name {
            "large_biomes" => Self::LargeBiomes,
            "amplified" => Self::Amplified,
            _ => Self::Default,
        }
    }

    fn temperature_noise(self) -> &'static str {
        if matches!(self, Self::LargeBiomes) {
            "minecraft:temperature_large"
        } else {
            "minecraft:temperature"
        }
    }

    fn vegetation_noise(self) -> &'static str {
        if matches!(self, Self::LargeBiomes) {
            "minecraft:vegetation_large"
        } else {
            "minecraft:vegetation"
        }
    }

    fn continentalness_noise(self) -> &'static str {
        if matches!(self, Self::LargeBiomes) {
            "minecraft:continentalness_large"
        } else {
            "minecraft:continentalness"
        }
    }

    fn erosion_noise(self) -> &'static str {
        if matches!(self, Self::LargeBiomes) {
            "minecraft:erosion_large"
        } else {
            "minecraft:erosion"
        }
    }

    fn temperature_first_octave(self) -> i32 {
        if matches!(self, Self::LargeBiomes) {
            TEMPERATURE_LARGE_FIRST_OCTAVE
        } else {
            -10
        }
    }

    fn vegetation_first_octave(self) -> i32 {
        if matches!(self, Self::LargeBiomes) {
            VEGETATION_LARGE_FIRST_OCTAVE
        } else {
            -8
        }
    }

    fn continentalness_first_octave(self) -> i32 {
        if matches!(self, Self::LargeBiomes) {
            CONTINENTALNESS_LARGE_FIRST_OCTAVE
        } else {
            -9
        }
    }

    fn erosion_first_octave(self) -> i32 {
        if matches!(self, Self::LargeBiomes) {
            EROSION_LARGE_FIRST_OCTAVE
        } else {
            -9
        }
    }

    fn offset_spline(self) -> &'static str {
        match self {
            Self::Default => OVERWORLD_OFFSET_SPLINE,
            Self::LargeBiomes => OVERWORLD_LARGE_OFFSET_SPLINE,
            Self::Amplified => OVERWORLD_AMPLIFIED_OFFSET_SPLINE,
        }
    }

    fn factor_spline(self) -> &'static str {
        match self {
            Self::Default => OVERWORLD_FACTOR_SPLINE,
            Self::LargeBiomes => OVERWORLD_LARGE_FACTOR_SPLINE,
            Self::Amplified => OVERWORLD_AMPLIFIED_FACTOR_SPLINE,
        }
    }

    fn jaggedness_spline(self) -> &'static str {
        match self {
            Self::Default => OVERWORLD_JAGGEDNESS_SPLINE,
            Self::LargeBiomes => OVERWORLD_LARGE_JAGGEDNESS_SPLINE,
            Self::Amplified => OVERWORLD_AMPLIFIED_JAGGEDNESS_SPLINE,
        }
    }
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
    pub(crate) fn with_kind(seed: i64, kind: OverworldNoiseKind) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let factory = random.fork_positional();
        Self {
            climate: if matches!(kind, OverworldNoiseKind::LargeBiomes) {
                OverworldClimateNoise::large_biomes(seed)
            } else {
                OverworldClimateNoise::new(seed)
            },
            jagged: NormalNoise::from_factory(
                &factory,
                "minecraft:jagged",
                -16,
                &JAGGED_AMPLITUDES,
            ),
            caves: CaveNoise::new(&factory),
            noodle: NoodleNoise::new(&factory),
            offset: terrain_spline_from_density_json(kind.offset_spline()),
            factor: terrain_spline_from_density_json(kind.factor_spline()),
            jaggedness: terrain_spline_from_density_json(kind.jaggedness_spline()),
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

    pub(crate) fn biome(&self, block_x: i32, block_y: i32, block_z: i32) -> &'static str {
        let climate = self.climate.sample(block_x, block_z);
        let profile = self.profile(block_x, block_z);
        select_overworld_biome(&climate, profile.depth(block_y))
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

    pub(crate) fn column_sampler<'a>(
        &'a self,
        profile: &'a OverworldTerrainProfile,
        block_x: i32,
        block_z: i32,
    ) -> OverworldTerrainColumn<'a> {
        OverworldTerrainColumn {
            noise: self,
            profile,
            block_x,
            block_z,
            jagged: profile.jaggedness
                * half_negative(self.jagged.get_value(
                    block_x as f64 * 1500.0,
                    0.0,
                    block_z as f64 * 1500.0,
                )),
        }
    }

    pub(crate) fn preliminary_surface_height(
        &self,
        profile: &OverworldTerrainProfile,
        _block_x: i32,
        _block_z: i32,
    ) -> i32 {
        let upper = map(
            0.273_437_5 / profile.factor - profile.offset,
            1.5,
            -1.5,
            -64.0,
            320.0,
        )
        .clamp(-40.0, 320.0);
        let top_y = (upper / 8.0).floor() as i32 * 8;
        if top_y <= -64 {
            return -64;
        }

        let mut y = top_y;
        while y >= -64 {
            if self.preliminary_surface_density(profile, y) > 0.0 {
                return y;
            }
            y -= 8;
        }
        -64
    }

    fn preliminary_surface_density(&self, profile: &OverworldTerrainProfile, block_y: i32) -> f64 {
        let depth = y_clamped_gradient(block_y, -64, 320, 1.5, -1.5) + profile.offset;
        let density =
            (4.0 * quarter_negative(depth * profile.factor) - 0.703_125).clamp(-64.0, 64.0);
        slide_overworld(block_y, density) - 0.390_625
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

#[derive(Clone, Debug)]
pub(crate) struct OverworldTerrainColumn<'a> {
    noise: &'a OverworldTerrainNoise,
    profile: &'a OverworldTerrainProfile,
    block_x: i32,
    block_z: i32,
    jagged: f64,
}

impl OverworldTerrainColumn<'_> {
    pub(crate) fn final_density(&self, block_y: i32, base_3d_noise: f64) -> f64 {
        let sloped_cheese = self.sloped_cheese_density(block_y, base_3d_noise);
        let entrances = self
            .noise
            .caves
            .entrances(self.block_x, block_y, self.block_z);
        let caves = if sloped_cheese < 1.5625 {
            sloped_cheese.min(5.0 * entrances)
        } else {
            self.noise.caves.underground_density(
                self.block_x,
                block_y,
                self.block_z,
                sloped_cheese,
                entrances,
            )
        };
        let post_processed = post_process_density(slide_overworld(block_y, caves));
        post_processed.min(
            self.noise
                .noodle
                .sample(self.block_x, block_y, self.block_z),
        )
    }

    fn sloped_cheese_density(&self, block_y: i32, base_3d_noise: f64) -> f64 {
        let depth = y_clamped_gradient(block_y, -64, 320, 1.5, -1.5) + self.profile.offset;
        4.0 * quarter_negative((depth + self.jagged) * self.profile.factor) + base_3d_noise
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverworldTerrainProfile {
    offset: f64,
    factor: f64,
    jaggedness: f64,
}

impl OverworldTerrainProfile {
    fn depth(&self, block_y: i32) -> f64 {
        y_clamped_gradient(block_y, -64, 320, 1.5, -1.5) + self.offset
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OverworldSurfaceRules {
    bedrock_floor: PositionalRandomFactory,
    deepslate: PositionalRandomFactory,
    surface: NormalNoise,
    swamp: NormalNoise,
    calcite: NormalNoise,
    gravel: NormalNoise,
    packed_ice: NormalNoise,
    ice: NormalNoise,
    powder_snow: NormalNoise,
    clay_bands_offset: NormalNoise,
    clay_bands: [SurfaceBlock; 192],
}

impl OverworldSurfaceRules {
    pub(crate) fn new(seed: i64) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let root = random.fork_positional();
        let mut bedrock_floor = root.from_hash_of("minecraft:bedrock_floor");
        let mut deepslate = root.from_hash_of("minecraft:deepslate");
        let mut clay_bands_random = root.from_hash_of("minecraft:clay_bands");
        Self {
            bedrock_floor: bedrock_floor.fork_positional(),
            deepslate: deepslate.fork_positional(),
            surface: NormalNoise::from_factory(&root, "minecraft:surface", -6, &SURFACE_AMPLITUDES),
            swamp: NormalNoise::from_factory(
                &root,
                "minecraft:surface_swamp",
                -2,
                &SURFACE_SWAMP_AMPLITUDES,
            ),
            calcite: NormalNoise::from_factory(&root, "minecraft:calcite", -9, &CALCITE_AMPLITUDES),
            gravel: NormalNoise::from_factory(&root, "minecraft:gravel", -8, &GRAVEL_AMPLITUDES),
            packed_ice: NormalNoise::from_factory(
                &root,
                "minecraft:packed_ice",
                -7,
                &PACKED_ICE_AMPLITUDES,
            ),
            ice: NormalNoise::from_factory(&root, "minecraft:ice", -4, &ICE_AMPLITUDES),
            powder_snow: NormalNoise::from_factory(
                &root,
                "minecraft:powder_snow",
                -6,
                &POWDER_SNOW_AMPLITUDES,
            ),
            clay_bands_offset: NormalNoise::from_factory(
                &root,
                "minecraft:clay_bands_offset",
                -8,
                &CLAY_BANDS_OFFSET_AMPLITUDES,
            ),
            clay_bands: generate_clay_bands(&mut clay_bands_random),
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

    pub(crate) fn block_at(&self, context: SurfaceRuleContext<'_>) -> Option<SurfaceBlock> {
        if context.y <= context.min_y {
            return Some(SurfaceBlock::Bedrock);
        }

        if self.is_bedrock_floor(context.x, context.y, context.z, context.min_y) {
            return Some(SurfaceBlock::Bedrock);
        }

        if context.y < context.surface_height - 8 {
            return self
                .is_deepslate(context.x, context.y, context.z)
                .then_some(SurfaceBlock::Deepslate);
        }

        let surface_depth = context.surface_height - context.y;
        if surface_depth < 0 {
            return None;
        }

        let on_floor = surface_depth == 0;
        let under_floor = (1..=3).contains(&surface_depth);
        let deep_under_floor = (4..=6).contains(&surface_depth);
        let very_deep_under_floor = surface_depth > 6;
        let above_water = context.above_water;
        let steep = context.slope > 2;

        if is_badlands(context.biome) {
            return self.badlands_block(context, on_floor, under_floor, above_water);
        }

        if on_floor {
            return Some(self.surface_block(context, above_water, steep));
        }

        if under_floor {
            return Some(self.under_surface_block(context, above_water, steep));
        }

        if deep_under_floor && has_sandstone_under_surface(context.biome) {
            return Some(SurfaceBlock::Sandstone);
        }

        if very_deep_under_floor && context.biome == "minecraft:desert" {
            return Some(SurfaceBlock::Sandstone);
        }

        self.is_deepslate(context.x, context.y, context.z)
            .then_some(SurfaceBlock::Deepslate)
    }

    fn surface_block(
        &self,
        context: SurfaceRuleContext<'_>,
        above_water: bool,
        steep: bool,
    ) -> SurfaceBlock {
        match context.biome {
            "minecraft:frozen_peaks" => self.frozen_peak_block(context, above_water, steep, true),
            "minecraft:snowy_slopes" => self.snowy_slope_block(context, above_water, steep, true),
            "minecraft:jagged_peaks" => {
                if steep {
                    SurfaceBlock::Stone
                } else if above_water {
                    SurfaceBlock::SnowBlock
                } else {
                    SurfaceBlock::Stone
                }
            }
            "minecraft:grove" => self
                .powder_snow_block(context, 0.35, 0.6)
                .unwrap_or(SurfaceBlock::SnowBlock),
            "minecraft:stony_peaks" => self.stony_peak_block(context),
            "minecraft:stony_shore" => self.stony_shore_block(context),
            "minecraft:windswept_hills" => {
                if self.surface_noise(context) > 1.0 {
                    SurfaceBlock::Stone
                } else {
                    self.grass_or_dirt(above_water)
                }
            }
            "minecraft:windswept_savanna" => {
                if self.surface_noise(context) > 1.75 {
                    SurfaceBlock::Stone
                } else if self.surface_noise(context) > -0.5 {
                    SurfaceBlock::CoarseDirt
                } else {
                    self.grass_or_dirt(above_water)
                }
            }
            "minecraft:windswept_gravelly_hills" => {
                let noise = self.surface_noise(context);
                if noise > 2.0 {
                    SurfaceBlock::Gravel
                } else if noise > 1.0 {
                    SurfaceBlock::Stone
                } else if noise > -1.0 {
                    self.grass_or_dirt(above_water)
                } else {
                    SurfaceBlock::Gravel
                }
            }
            "minecraft:old_growth_pine_taiga" | "minecraft:old_growth_spruce_taiga" => {
                let noise = self.surface_noise(context);
                if noise > 1.75 {
                    SurfaceBlock::CoarseDirt
                } else if noise > -0.95 {
                    SurfaceBlock::Podzol
                } else {
                    self.grass_or_dirt(above_water)
                }
            }
            "minecraft:ice_spikes" => {
                if above_water {
                    SurfaceBlock::SnowBlock
                } else {
                    self.grass_or_dirt(above_water)
                }
            }
            "minecraft:mangrove_swamp" => SurfaceBlock::Mud,
            "minecraft:swamp" => self.swamp_surface_block(context),
            "minecraft:mushroom_fields" => SurfaceBlock::Mycelium,
            "minecraft:warm_ocean"
            | "minecraft:beach"
            | "minecraft:snowy_beach"
            | "minecraft:desert" => SurfaceBlock::Sand,
            "minecraft:lukewarm_ocean" | "minecraft:deep_lukewarm_ocean" => SurfaceBlock::Sand,
            "minecraft:dripstone_caves" => SurfaceBlock::Stone,
            _ => self.grass_or_dirt(above_water),
        }
    }

    fn under_surface_block(
        &self,
        context: SurfaceRuleContext<'_>,
        above_water: bool,
        steep: bool,
    ) -> SurfaceBlock {
        match context.biome {
            "minecraft:frozen_peaks" => self.frozen_peak_block(context, above_water, steep, false),
            "minecraft:snowy_slopes" => self.snowy_slope_block(context, above_water, steep, false),
            "minecraft:jagged_peaks" => SurfaceBlock::Stone,
            "minecraft:grove" => self
                .powder_snow_block(context, 0.45, 0.58)
                .unwrap_or(SurfaceBlock::Dirt),
            "minecraft:stony_peaks" => self.stony_peak_block(context),
            "minecraft:stony_shore" => self.stony_shore_block(context),
            "minecraft:windswept_savanna" => {
                if self.surface_noise(context) > 1.75 {
                    SurfaceBlock::Stone
                } else {
                    SurfaceBlock::Dirt
                }
            }
            "minecraft:windswept_gravelly_hills" => {
                let noise = self.surface_noise(context);
                if noise > 2.0 {
                    SurfaceBlock::Gravel
                } else if noise > 1.0 {
                    SurfaceBlock::Stone
                } else if noise > -1.0 {
                    SurfaceBlock::Dirt
                } else {
                    SurfaceBlock::Gravel
                }
            }
            "minecraft:mangrove_swamp" => SurfaceBlock::Mud,
            "minecraft:warm_ocean"
            | "minecraft:beach"
            | "minecraft:snowy_beach"
            | "minecraft:desert"
            | "minecraft:lukewarm_ocean"
            | "minecraft:deep_lukewarm_ocean" => SurfaceBlock::Sand,
            "minecraft:dripstone_caves" => SurfaceBlock::Stone,
            _ => SurfaceBlock::Dirt,
        }
    }

    fn frozen_peak_block(
        &self,
        context: SurfaceRuleContext<'_>,
        above_water: bool,
        steep: bool,
        on_surface: bool,
    ) -> SurfaceBlock {
        if steep || self.packed_ice_noise(context, on_surface) {
            return SurfaceBlock::PackedIce;
        }
        if self.ice_noise(context, on_surface) {
            return SurfaceBlock::Ice;
        }
        if above_water {
            SurfaceBlock::SnowBlock
        } else {
            SurfaceBlock::Stone
        }
    }

    fn snowy_slope_block(
        &self,
        context: SurfaceRuleContext<'_>,
        above_water: bool,
        steep: bool,
        on_surface: bool,
    ) -> SurfaceBlock {
        if steep {
            SurfaceBlock::Stone
        } else {
            let (min, max) = if on_surface {
                (0.35, 0.6)
            } else {
                (0.45, 0.58)
            };
            self.powder_snow_block(context, min, max)
                .unwrap_or(if above_water {
                    SurfaceBlock::SnowBlock
                } else {
                    SurfaceBlock::Dirt
                })
        }
    }

    fn stony_peak_block(&self, context: SurfaceRuleContext<'_>) -> SurfaceBlock {
        if self.noise_between(&self.calcite, context, -0.0125, 0.0125) {
            SurfaceBlock::Calcite
        } else {
            SurfaceBlock::Stone
        }
    }

    fn stony_shore_block(&self, context: SurfaceRuleContext<'_>) -> SurfaceBlock {
        if self.noise_between(&self.gravel, context, -0.05, 0.05) {
            SurfaceBlock::Gravel
        } else {
            SurfaceBlock::Stone
        }
    }

    fn swamp_surface_block(&self, context: SurfaceRuleContext<'_>) -> SurfaceBlock {
        let puddle_y = if context.biome == "minecraft:mangrove_swamp" {
            60
        } else {
            62
        };
        if context.y <= puddle_y && context.y < context.sea_level && self.swamp_noise(context) > 0.0
        {
            SurfaceBlock::Water
        } else {
            self.grass_or_dirt(context.above_water)
        }
    }

    fn badlands_block(
        &self,
        context: SurfaceRuleContext<'_>,
        on_floor: bool,
        under_floor: bool,
        above_water: bool,
    ) -> Option<SurfaceBlock> {
        if on_floor {
            if context.biome == "minecraft:wooded_badlands" && context.y >= 97 {
                let surface = self.surface_noise(context);
                if (-0.909..=-0.5454).contains(&surface)
                    || (-0.1818..=0.1818).contains(&surface)
                    || (0.5454..=0.909).contains(&surface)
                {
                    return Some(SurfaceBlock::CoarseDirt);
                }
                return Some(self.grass_or_dirt(above_water));
            }

            if context.y >= 256 {
                return Some(SurfaceBlock::OrangeTerracotta);
            }

            if context.y >= 74 {
                let surface = self.surface_noise(context);
                if (-0.909..=-0.5454).contains(&surface)
                    || (-0.1818..=0.1818).contains(&surface)
                    || (0.5454..=0.909).contains(&surface)
                {
                    return Some(SurfaceBlock::Terracotta);
                }
                return Some(self.clay_band(context.x, context.y, context.z));
            }

            if above_water {
                return Some(SurfaceBlock::RedSand);
            }

            return Some(SurfaceBlock::OrangeTerracotta);
        }

        if context.y >= 63 {
            if context.y < 74 {
                return Some(SurfaceBlock::OrangeTerracotta);
            }
            return Some(self.clay_band(context.x, context.y, context.z));
        }

        if under_floor {
            return Some(SurfaceBlock::WhiteTerracotta);
        }

        None
    }

    fn grass_or_dirt(&self, above_water: bool) -> SurfaceBlock {
        if above_water {
            SurfaceBlock::GrassBlock
        } else {
            SurfaceBlock::Dirt
        }
    }

    fn powder_snow_block(
        &self,
        context: SurfaceRuleContext<'_>,
        min: f64,
        max: f64,
    ) -> Option<SurfaceBlock> {
        (self.noise_between(&self.powder_snow, context, min, max) && context.y >= context.sea_level)
            .then_some(SurfaceBlock::PowderSnow)
    }

    fn packed_ice_noise(&self, context: SurfaceRuleContext<'_>, on_surface: bool) -> bool {
        let min = if on_surface { 0.0 } else { -0.5 };
        self.noise_between(&self.packed_ice, context, min, 0.2)
    }

    fn ice_noise(&self, context: SurfaceRuleContext<'_>, on_surface: bool) -> bool {
        let min = if on_surface { 0.0 } else { -0.0625 };
        self.noise_between(&self.ice, context, min, 0.025)
    }

    fn surface_noise(&self, context: SurfaceRuleContext<'_>) -> f64 {
        self.surface
            .get_value(context.x as f64, 0.0, context.z as f64)
    }

    fn swamp_noise(&self, context: SurfaceRuleContext<'_>) -> f64 {
        self.swamp
            .get_value(context.x as f64, 0.0, context.z as f64)
    }

    fn noise_between(
        &self,
        noise: &NormalNoise,
        context: SurfaceRuleContext<'_>,
        min: f64,
        max: f64,
    ) -> bool {
        let value = noise.get_value(context.x as f64, 0.0, context.z as f64);
        (min..=max).contains(&value)
    }

    fn clay_band(&self, x: i32, y: i32, z: i32) -> SurfaceBlock {
        let offset =
            (self.clay_bands_offset.get_value(x as f64, 0.0, z as f64) * 4.0).round() as i32;
        let index = (y + offset).rem_euclid(self.clay_bands.len() as i32) as usize;
        self.clay_bands[index]
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SurfaceRuleContext<'a> {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
    pub(crate) surface_height: i32,
    pub(crate) above_water: bool,
    pub(crate) sea_level: i32,
    pub(crate) min_y: i32,
    pub(crate) biome: &'a str,
    pub(crate) slope: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceBlock {
    Bedrock,
    Stone,
    Deepslate,
    Dirt,
    GrassBlock,
    Podzol,
    CoarseDirt,
    Mycelium,
    Calcite,
    Gravel,
    Sand,
    Sandstone,
    PackedIce,
    Ice,
    SnowBlock,
    PowderSnow,
    Mud,
    Water,
    Terracotta,
    OrangeTerracotta,
    WhiteTerracotta,
    YellowTerracotta,
    BrownTerracotta,
    RedTerracotta,
    LightGrayTerracotta,
    RedSand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AquiferFluid {
    Air,
    Water,
    Lava,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AquiferSubstance {
    DefaultBlock,
    Fluid(AquiferFluid),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FluidStatus {
    level: i32,
    fluid: AquiferFluid,
}

impl FluidStatus {
    fn at(self, block_y: i32) -> AquiferFluid {
        if block_y < self.level {
            self.fluid
        } else {
            AquiferFluid::Air
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OreVeinBlock {
    CopperOre,
    RawCopperBlock,
    Granite,
    DeepslateIronOre,
    RawIronBlock,
    Tuff,
}

#[derive(Clone, Copy, Debug)]
enum OreVeinType {
    Copper,
    Iron,
}

impl OreVeinType {
    fn min_y(self) -> i32 {
        match self {
            Self::Copper => 0,
            Self::Iron => -60,
        }
    }

    fn max_y(self) -> i32 {
        match self {
            Self::Copper => 50,
            Self::Iron => -8,
        }
    }

    fn ore(self) -> OreVeinBlock {
        match self {
            Self::Copper => OreVeinBlock::CopperOre,
            Self::Iron => OreVeinBlock::DeepslateIronOre,
        }
    }

    fn raw_ore_block(self) -> OreVeinBlock {
        match self {
            Self::Copper => OreVeinBlock::RawCopperBlock,
            Self::Iron => OreVeinBlock::RawIronBlock,
        }
    }

    fn filler(self) -> OreVeinBlock {
        match self {
            Self::Copper => OreVeinBlock::Granite,
            Self::Iron => OreVeinBlock::Tuff,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OreVeinNoise {
    veininess: NormalNoise,
    vein_a: NormalNoise,
    vein_b: NormalNoise,
    gap: NormalNoise,
    random: PositionalRandomFactory,
}

impl OreVeinNoise {
    pub(crate) fn new(seed: i64) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let root = random.fork_positional();
        let mut ore_random = root.from_hash_of("minecraft:ore");
        Self {
            veininess: NormalNoise::from_factory(
                &root,
                "minecraft:ore_veininess",
                -8,
                &ORE_VEININESS_AMPLITUDES,
            ),
            vein_a: NormalNoise::from_factory(
                &root,
                "minecraft:ore_vein_a",
                -7,
                &ORE_VEIN_RIDGE_AMPLITUDES,
            ),
            vein_b: NormalNoise::from_factory(
                &root,
                "minecraft:ore_vein_b",
                -7,
                &ORE_VEIN_RIDGE_AMPLITUDES,
            ),
            gap: NormalNoise::from_factory(&root, "minecraft:ore_gap", -5, &ORE_GAP_AMPLITUDES),
            random: ore_random.fork_positional(),
        }
    }

    pub(crate) fn block_at(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
    ) -> Option<OreVeinBlock> {
        if !(-60..=50).contains(&block_y) {
            return None;
        }

        let x = block_x as f64;
        let y = block_y as f64;
        let z = block_z as f64;
        let veininess = self.veininess.get_value(x * 1.5, y * 1.5, z * 1.5);
        let vein_type = if veininess > 0.0 {
            OreVeinType::Copper
        } else {
            OreVeinType::Iron
        };
        let ridged = veininess.abs();
        let distance_from_top = vein_type.max_y() - block_y;
        let distance_from_bottom = block_y - vein_type.min_y();
        if distance_from_bottom < 0 || distance_from_top < 0 {
            return None;
        }

        let distance_from_edge = distance_from_top.min(distance_from_bottom);
        let edge_roundoff = clamped_map(distance_from_edge as f64, 0.0, 20.0, -0.2, 0.0);
        if ridged + edge_roundoff < 0.4 {
            return None;
        }

        let mut random = self.random.at(block_x, block_y, block_z);
        if random.next_float() > 0.7 {
            return None;
        }

        let ridge_a = self.vein_a.get_value(x * 4.0, y * 4.0, z * 4.0).abs();
        let ridge_b = self.vein_b.get_value(x * 4.0, y * 4.0, z * 4.0).abs();
        if -0.08 + ridge_a.max(ridge_b) >= 0.0 {
            return None;
        }

        let richness = clamped_map(ridged, 0.4, 0.6, 0.1, 0.3);
        let gap = self.gap.get_value(x, y, z);
        if random.next_float() < richness as f32 && gap > -0.3 {
            if random.next_float() < 0.02 {
                Some(vein_type.raw_ore_block())
            } else {
                Some(vein_type.ore())
            }
        } else {
            Some(vein_type.filler())
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OverworldAquifer {
    barrier: NormalNoise,
    fluid_level_floodedness: NormalNoise,
    fluid_level_spread: NormalNoise,
    lava: NormalNoise,
    random: PositionalRandomFactory,
    sea_level: i32,
}

impl OverworldAquifer {
    pub(crate) fn new(seed: i64, sea_level: i32) -> Self {
        let mut random = XoroshiroRandomSource::new(seed);
        let factory = random.fork_positional();
        let mut aquifer_random = factory.from_hash_of("minecraft:aquifer");
        Self {
            barrier: NormalNoise::from_factory(
                &factory,
                "minecraft:aquifer_barrier",
                -3,
                &AQUIFER_BARRIER_AMPLITUDES,
            ),
            fluid_level_floodedness: NormalNoise::from_factory(
                &factory,
                "minecraft:aquifer_fluid_level_floodedness",
                -7,
                &AQUIFER_FLOODEDNESS_AMPLITUDES,
            ),
            fluid_level_spread: NormalNoise::from_factory(
                &factory,
                "minecraft:aquifer_fluid_level_spread",
                -5,
                &AQUIFER_SPREAD_AMPLITUDES,
            ),
            lava: NormalNoise::from_factory(
                &factory,
                "minecraft:aquifer_lava",
                -1,
                &AQUIFER_LAVA_AMPLITUDES,
            ),
            random: aquifer_random.fork_positional(),
            sea_level,
        }
    }

    pub(crate) fn substance_at(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        density: f64,
        preliminary_surface: i32,
    ) -> AquiferSubstance {
        if density > 0.0 {
            return AquiferSubstance::DefaultBlock;
        }

        let global_status = self.global_status(block_y);
        let global_fluid = global_status.at(block_y);
        if matches!(global_fluid, AquiferFluid::Lava) {
            return AquiferSubstance::Fluid(AquiferFluid::Lava);
        }

        let nearest =
            self.nearest_aquifer_locations(block_x, block_y, block_z, preliminary_surface);
        let closest_status = nearest[0].status;
        let similarity12 = aquifer_similarity(nearest[0].distance_sqr, nearest[1].distance_sqr);
        let fluid = closest_status.at(block_y);
        if similarity12 <= 0.0 {
            return AquiferSubstance::Fluid(fluid);
        }

        if matches!(fluid, AquiferFluid::Water)
            && matches!(
                self.global_status(block_y - 1).at(block_y - 1),
                AquiferFluid::Lava
            )
        {
            return AquiferSubstance::Fluid(fluid);
        }

        let barrier12 = similarity12
            * self.calculate_pressure(block_x, block_y, block_z, closest_status, nearest[1].status);
        if density + barrier12 > 0.0 {
            return AquiferSubstance::DefaultBlock;
        }

        let similarity13 = aquifer_similarity(nearest[0].distance_sqr, nearest[2].distance_sqr);
        if similarity13 > 0.0 {
            let barrier13 = similarity12
                * similarity13
                * self.calculate_pressure(
                    block_x,
                    block_y,
                    block_z,
                    closest_status,
                    nearest[2].status,
                );
            if density + barrier13 > 0.0 {
                return AquiferSubstance::DefaultBlock;
            }
        }

        let similarity23 = aquifer_similarity(nearest[1].distance_sqr, nearest[2].distance_sqr);
        if similarity23 > 0.0 {
            let barrier23 = similarity12
                * similarity23
                * self.calculate_pressure(
                    block_x,
                    block_y,
                    block_z,
                    nearest[1].status,
                    nearest[2].status,
                );
            if density + barrier23 > 0.0 {
                return AquiferSubstance::DefaultBlock;
            }
        }

        AquiferSubstance::Fluid(fluid)
    }

    fn nearest_aquifer_locations(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        preliminary_surface: i32,
    ) -> [AquiferSample; 4] {
        let x_anchor = grid_x(block_x - 5);
        let y_anchor = grid_y(block_y + 1);
        let z_anchor = grid_z(block_z - 5);
        let mut nearest = [AquiferSample::empty(); 4];

        for x_offset in 0..=1 {
            for y_offset in -1..=1 {
                for z_offset in 0..=1 {
                    let cell_x = x_anchor + x_offset;
                    let cell_y = y_anchor + y_offset;
                    let cell_z = z_anchor + z_offset;
                    let location = self.aquifer_location(cell_x, cell_y, cell_z);
                    let dx = location.x - block_x;
                    let dy = location.y - block_y;
                    let dz = location.z - block_z;
                    let distance_sqr = dx * dx + dy * dy + dz * dz;
                    let status = self.compute_status(
                        location.x,
                        location.y,
                        location.z,
                        preliminary_surface,
                    );
                    insert_aquifer_sample(
                        &mut nearest,
                        AquiferSample {
                            distance_sqr,
                            status,
                        },
                    );
                }
            }
        }

        nearest
    }

    fn aquifer_location(&self, cell_x: i32, cell_y: i32, cell_z: i32) -> AquiferLocation {
        let mut random = self.random.at(cell_x, cell_y, cell_z);
        AquiferLocation {
            x: from_grid_x(cell_x, random.next_int(10) as i32),
            y: from_grid_y(cell_y, random.next_int(9) as i32),
            z: from_grid_z(cell_z, random.next_int(10) as i32),
        }
    }

    fn compute_status(&self, x: i32, y: i32, z: i32, preliminary_surface: i32) -> FluidStatus {
        let global_status = self.global_status(y);
        let global_fluid = global_status.at(y);
        if matches!(global_fluid, AquiferFluid::Lava) {
            return global_status;
        }

        let adjusted_surface = preliminary_surface + 8;
        let top_of_cell = y + 12;
        let bottom_of_cell = y - 12;
        if bottom_of_cell > adjusted_surface {
            return global_status;
        }

        let global_at_surface = self.global_status(adjusted_surface);
        let surface_is_under_global_fluid =
            !matches!(global_at_surface.at(adjusted_surface), AquiferFluid::Air);
        if top_of_cell > adjusted_surface && surface_is_under_global_fluid {
            return global_at_surface;
        }

        let surface_level = self.fluid_surface_level(
            x,
            y,
            z,
            preliminary_surface,
            global_status.level,
            surface_is_under_global_fluid,
        );
        FluidStatus {
            level: surface_level,
            fluid: self.fluid_type(x, y, z, global_status.fluid, surface_level),
        }
    }

    fn global_status(&self, block_y: i32) -> FluidStatus {
        if block_y < self.global_lava_cutoff() {
            FluidStatus {
                level: -54,
                fluid: AquiferFluid::Lava,
            }
        } else {
            FluidStatus {
                level: self.sea_level,
                fluid: AquiferFluid::Water,
            }
        }
    }

    fn calculate_pressure(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        first: FluidStatus,
        second: FluidStatus,
    ) -> f64 {
        let type_1 = first.at(block_y);
        let type_2 = second.at(block_y);
        if matches!(
            (type_1, type_2),
            (AquiferFluid::Lava, AquiferFluid::Water) | (AquiferFluid::Water, AquiferFluid::Lava)
        ) {
            return 2.0;
        }

        let fluid_y_diff = (first.level - second.level).abs();
        if fluid_y_diff == 0 {
            return 0.0;
        }

        let average_fluid_y = 0.5 * (first.level + second.level) as f64;
        let above_average = block_y as f64 + 0.5 - average_fluid_y;
        let distance_from_edge = fluid_y_diff as f64 / 2.0 - above_average.abs();
        let gradient = if above_average > 0.0 {
            let center = distance_from_edge;
            if center > 0.0 {
                center / 1.5
            } else {
                center / 2.5
            }
        } else {
            let center = 3.0 + distance_from_edge;
            if center > 0.0 {
                center / 3.0
            } else {
                center / 10.0
            }
        };
        let noise = if (-2.0..=2.0).contains(&gradient) {
            self.barrier
                .get_value(block_x as f64, block_y as f64 * 0.5, block_z as f64)
        } else {
            0.0
        };
        2.0 * (noise + gradient)
    }

    fn global_lava_cutoff(&self) -> i32 {
        self.sea_level.min(-54)
    }

    fn fluid_surface_level(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        preliminary_surface: i32,
        global_level: i32,
        surface_is_under_global_fluid: bool,
    ) -> i32 {
        let adjusted_surface = preliminary_surface + 8;
        let distance_below_surface = adjusted_surface - block_y;
        let floodedness_factor = if surface_is_under_global_fluid {
            clamped_map(distance_below_surface as f64, 0.0, 64.0, 1.0, 0.0)
        } else {
            0.0
        };
        let floodedness = self
            .fluid_level_floodedness
            .get_value(block_x as f64, block_y as f64 * 0.67, block_z as f64)
            .clamp(-1.0, 1.0);
        let fully_flooded_threshold = map(floodedness_factor, 1.0, 0.0, -0.3, 0.8);
        let partially_flooded_threshold = map(floodedness_factor, 1.0, 0.0, -0.8, 0.4);

        if floodedness - fully_flooded_threshold > 0.0 {
            global_level
        } else if floodedness - partially_flooded_threshold > 0.0 {
            self.randomized_fluid_surface_level(block_x, block_y, block_z, preliminary_surface)
        } else {
            i32::MIN / 2
        }
    }

    fn randomized_fluid_surface_level(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        preliminary_surface: i32,
    ) -> i32 {
        let cell_x = block_x.div_euclid(16);
        let cell_y = block_y.div_euclid(40);
        let cell_z = block_z.div_euclid(16);
        let cell_middle_y = cell_y * 40 + 20;
        let spread = self.fluid_level_spread.get_value(
            cell_x as f64,
            cell_y as f64 * (5.0 / 7.0),
            cell_z as f64,
        ) * 10.0;
        let target = cell_middle_y + quantize(spread, 3);
        preliminary_surface.min(target)
    }

    fn fluid_type(
        &self,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        global_fluid: AquiferFluid,
        fluid_level: i32,
    ) -> AquiferFluid {
        if fluid_level > -10 || matches!(global_fluid, AquiferFluid::Lava) {
            return global_fluid;
        }

        let cell_x = block_x.div_euclid(64);
        let cell_y = block_y.div_euclid(40);
        let cell_z = block_z.div_euclid(64);
        let lava = self
            .lava
            .get_value(cell_x as f64, cell_y as f64, cell_z as f64);
        if lava.abs() > 0.3 {
            AquiferFluid::Lava
        } else {
            global_fluid
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AquiferLocation {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Clone, Copy, Debug)]
struct AquiferSample {
    distance_sqr: i32,
    status: FluidStatus,
}

impl AquiferSample {
    fn empty() -> Self {
        Self {
            distance_sqr: i32::MAX,
            status: FluidStatus {
                level: i32::MIN / 2,
                fluid: AquiferFluid::Air,
            },
        }
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
            "minecraft:overworld/continents" | "minecraft:overworld_large_biomes/continents" => {
                Self::Continents
            }
            "minecraft:overworld/erosion" | "minecraft:overworld_large_biomes/erosion" => {
                Self::Erosion
            }
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
pub(crate) struct XoroshiroRandomSource {
    random: Xoroshiro128PlusPlus,
}

impl XoroshiroRandomSource {
    pub(crate) fn new(seed: i64) -> Self {
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

    pub(crate) fn next_long(&mut self) -> u64 {
        self.random.next_long()
    }

    pub(crate) fn next_int(&mut self, bound: usize) -> usize {
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

    pub(crate) fn next_double(&mut self) -> f64 {
        (self.random.next_long() >> 11) as f64 * (1.110_223e-16_f32 as f64)
    }

    pub(crate) fn next_float(&mut self) -> f32 {
        (self.random.next_long() >> 40) as f32 * 5.960_464_5e-8_f32
    }

    fn next_bool(&mut self) -> bool {
        (self.random.next_long() & 1) != 0
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

fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    lerp((value - from_min) / (from_max - from_min), to_min, to_max)
}

fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    clamped_lerp((value - from_min) / (from_max - from_min), to_min, to_max)
}

fn quantize(value: f64, resolution: i32) -> i32 {
    (value.floor() as i32).div_euclid(resolution) * resolution
}

fn aquifer_similarity(distance_sqr_1: i32, distance_sqr_2: i32) -> f64 {
    1.0 - (distance_sqr_2 - distance_sqr_1) as f64 / 25.0
}

fn insert_aquifer_sample(samples: &mut [AquiferSample; 4], sample: AquiferSample) {
    if samples[0].distance_sqr >= sample.distance_sqr {
        samples[3] = samples[2];
        samples[2] = samples[1];
        samples[1] = samples[0];
        samples[0] = sample;
    } else if samples[1].distance_sqr >= sample.distance_sqr {
        samples[3] = samples[2];
        samples[2] = samples[1];
        samples[1] = sample;
    } else if samples[2].distance_sqr >= sample.distance_sqr {
        samples[3] = samples[2];
        samples[2] = sample;
    } else if samples[3].distance_sqr >= sample.distance_sqr {
        samples[3] = sample;
    }
}

fn grid_x(block_x: i32) -> i32 {
    block_x >> 4
}

fn from_grid_x(grid_x: i32, offset: i32) -> i32 {
    (grid_x << 4) + offset
}

fn grid_y(block_y: i32) -> i32 {
    block_y.div_euclid(12)
}

fn from_grid_y(grid_y: i32, offset: i32) -> i32 {
    grid_y * 12 + offset
}

fn grid_z(block_z: i32) -> i32 {
    block_z >> 4
}

fn from_grid_z(grid_z: i32, offset: i32) -> i32 {
    (grid_z << 4) + offset
}

fn select_overworld_biome(climate: &OverworldClimateSample, depth: f64) -> &'static str {
    let target = ClimateTarget::new(
        climate.temperature,
        climate.vegetation,
        climate.continentalness,
        climate.erosion,
        depth,
        climate.ridges,
    );
    overworld_biome_points()
        .iter()
        .min_by_key(|point| point.fitness(target))
        .map(|point| point.biome)
        .unwrap_or("minecraft:plains")
}

#[derive(Clone, Copy, Debug)]
struct ClimateParameter {
    min: i64,
    max: i64,
}

impl ClimateParameter {
    fn point(value: f64) -> Self {
        Self::span(value, value)
    }

    fn span(min: f64, max: f64) -> Self {
        Self {
            min: quantize_climate(min),
            max: quantize_climate(max),
        }
    }

    fn span_parameters(min: Self, max: Self) -> Self {
        Self {
            min: min.min,
            max: max.max,
        }
    }

    fn distance(self, target: i64) -> i64 {
        let above = target - self.max;
        let below = self.min - target;
        if above > 0 { above } else { below.max(0) }
    }
}

#[derive(Clone, Copy, Debug)]
struct ClimateTarget {
    values: [i64; 6],
}

impl ClimateTarget {
    fn new(
        temperature: f64,
        humidity: f64,
        continentalness: f64,
        erosion: f64,
        depth: f64,
        weirdness: f64,
    ) -> Self {
        Self {
            values: [
                quantize_climate(temperature),
                quantize_climate(humidity),
                quantize_climate(continentalness),
                quantize_climate(erosion),
                quantize_climate(depth),
                quantize_climate(weirdness),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BiomeParameterPoint {
    parameters: [ClimateParameter; 6],
    offset: i64,
    biome: &'static str,
}

impl BiomeParameterPoint {
    fn fitness(self, target: ClimateTarget) -> i64 {
        self.parameters
            .iter()
            .zip(target.values)
            .map(|(parameter, target)| {
                let distance = parameter.distance(target);
                distance * distance
            })
            .sum::<i64>()
            + self.offset * self.offset
    }
}

#[derive(Clone, Debug)]
struct OverworldBiomeBuilder {
    full_range: ClimateParameter,
    temperatures: [ClimateParameter; 5],
    humidities: [ClimateParameter; 5],
    erosions: [ClimateParameter; 7],
    frozen_range: ClimateParameter,
    unfrozen_range: ClimateParameter,
    mushroom_fields_continentalness: ClimateParameter,
    deep_ocean_continentalness: ClimateParameter,
    ocean_continentalness: ClimateParameter,
    coast_continentalness: ClimateParameter,
    inland_continentalness: ClimateParameter,
    near_inland_continentalness: ClimateParameter,
    mid_inland_continentalness: ClimateParameter,
    far_inland_continentalness: ClimateParameter,
}

impl OverworldBiomeBuilder {
    fn new() -> Self {
        let full_range = ClimateParameter::span(-1.0, 1.0);
        let temperatures = [
            ClimateParameter::span(-1.0, -0.45),
            ClimateParameter::span(-0.45, -0.15),
            ClimateParameter::span(-0.15, 0.2),
            ClimateParameter::span(0.2, 0.55),
            ClimateParameter::span(0.55, 1.0),
        ];
        let humidities = [
            ClimateParameter::span(-1.0, -0.35),
            ClimateParameter::span(-0.35, -0.1),
            ClimateParameter::span(-0.1, 0.1),
            ClimateParameter::span(0.1, 0.3),
            ClimateParameter::span(0.3, 1.0),
        ];
        let erosions = [
            ClimateParameter::span(-1.0, -0.78),
            ClimateParameter::span(-0.78, -0.375),
            ClimateParameter::span(-0.375, -0.2225),
            ClimateParameter::span(-0.2225, 0.05),
            ClimateParameter::span(0.05, 0.45),
            ClimateParameter::span(0.45, 0.55),
            ClimateParameter::span(0.55, 1.0),
        ];

        Self {
            full_range,
            temperatures,
            humidities,
            erosions,
            frozen_range: temperatures[0],
            unfrozen_range: ClimateParameter::span_parameters(temperatures[1], temperatures[4]),
            mushroom_fields_continentalness: ClimateParameter::span(-1.2, -1.05),
            deep_ocean_continentalness: ClimateParameter::span(-1.05, -0.455),
            ocean_continentalness: ClimateParameter::span(-0.455, -0.19),
            coast_continentalness: ClimateParameter::span(-0.19, -0.11),
            inland_continentalness: ClimateParameter::span(-0.11, 0.55),
            near_inland_continentalness: ClimateParameter::span(-0.11, 0.03),
            mid_inland_continentalness: ClimateParameter::span(0.03, 0.3),
            far_inland_continentalness: ClimateParameter::span(0.3, 1.0),
        }
    }

    fn build(self) -> Vec<BiomeParameterPoint> {
        let mut points = Vec::with_capacity(900);
        self.add_off_coast_biomes(&mut points);
        self.add_inland_biomes(&mut points);
        self.add_underground_biomes(&mut points);
        points
    }

    fn add_off_coast_biomes(&self, points: &mut Vec<BiomeParameterPoint>) {
        self.add_surface_biome(
            points,
            self.full_range,
            self.full_range,
            self.mushroom_fields_continentalness,
            self.full_range,
            self.full_range,
            0.0,
            "minecraft:mushroom_fields",
        );

        for temperature_index in 0..self.temperatures.len() {
            let temperature = self.temperatures[temperature_index];
            self.add_surface_biome(
                points,
                temperature,
                self.full_range,
                self.deep_ocean_continentalness,
                self.full_range,
                self.full_range,
                0.0,
                OCEANS[0][temperature_index],
            );
            self.add_surface_biome(
                points,
                temperature,
                self.full_range,
                self.ocean_continentalness,
                self.full_range,
                self.full_range,
                0.0,
                OCEANS[1][temperature_index],
            );
        }
    }

    fn add_inland_biomes(&self, points: &mut Vec<BiomeParameterPoint>) {
        self.add_mid_slice(points, ClimateParameter::span(-1.0, -0.93333334));
        self.add_high_slice(points, ClimateParameter::span(-0.93333334, -0.7666667));
        self.add_peaks(points, ClimateParameter::span(-0.7666667, -0.56666666));
        self.add_high_slice(points, ClimateParameter::span(-0.56666666, -0.4));
        self.add_mid_slice(points, ClimateParameter::span(-0.4, -0.26666668));
        self.add_low_slice(points, ClimateParameter::span(-0.26666668, -0.05));
        self.add_valleys(points, ClimateParameter::span(-0.05, 0.05));
        self.add_low_slice(points, ClimateParameter::span(0.05, 0.26666668));
        self.add_mid_slice(points, ClimateParameter::span(0.26666668, 0.4));
        self.add_high_slice(points, ClimateParameter::span(0.4, 0.56666666));
        self.add_peaks(points, ClimateParameter::span(0.56666666, 0.7666667));
        self.add_high_slice(points, ClimateParameter::span(0.7666667, 0.93333334));
        self.add_mid_slice(points, ClimateParameter::span(0.93333334, 1.0));
    }

    fn add_peaks(&self, points: &mut Vec<BiomeParameterPoint>, weirdness: ClimateParameter) {
        for temperature_index in 0..self.temperatures.len() {
            let temperature = self.temperatures[temperature_index];
            for humidity_index in 0..self.humidities.len() {
                let humidity = self.humidities[humidity_index];
                let middle_biome =
                    self.pick_middle_biome(temperature_index, humidity_index, weirdness);
                let middle_or_badlands = self.pick_middle_biome_or_badlands_if_hot(
                    temperature_index,
                    humidity_index,
                    weirdness,
                );
                let middle_badlands_or_slope = self
                    .pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(
                        temperature_index,
                        humidity_index,
                        weirdness,
                    );
                let plateau_biome =
                    self.pick_plateau_biome(temperature_index, humidity_index, weirdness);
                let shattered_biome =
                    self.pick_shattered_biome(temperature_index, humidity_index, weirdness);
                let shattered_or_savanna = self.maybe_pick_windswept_savanna_biome(
                    temperature_index,
                    humidity_index,
                    weirdness,
                    shattered_biome,
                );
                let peak_biome = self.pick_peak_biome(temperature_index, humidity_index, weirdness);
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[0],
                    weirdness,
                    0.0,
                    peak_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    self.erosions[1],
                    weirdness,
                    0.0,
                    middle_badlands_or_slope,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[1],
                    weirdness,
                    0.0,
                    peak_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    ClimateParameter::span_parameters(self.erosions[2], self.erosions[3]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[2],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.mid_inland_continentalness,
                    self.erosions[3],
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    self.erosions[3],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[4],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    self.erosions[5],
                    weirdness,
                    0.0,
                    shattered_or_savanna,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[5],
                    weirdness,
                    0.0,
                    shattered_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[6],
                    weirdness,
                    0.0,
                    middle_biome,
                );
            }
        }
    }

    fn add_high_slice(&self, points: &mut Vec<BiomeParameterPoint>, weirdness: ClimateParameter) {
        for temperature_index in 0..self.temperatures.len() {
            let temperature = self.temperatures[temperature_index];
            for humidity_index in 0..self.humidities.len() {
                let humidity = self.humidities[humidity_index];
                let middle_biome =
                    self.pick_middle_biome(temperature_index, humidity_index, weirdness);
                let middle_or_badlands = self.pick_middle_biome_or_badlands_if_hot(
                    temperature_index,
                    humidity_index,
                    weirdness,
                );
                let middle_badlands_or_slope = self
                    .pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(
                        temperature_index,
                        humidity_index,
                        weirdness,
                    );
                let plateau_biome =
                    self.pick_plateau_biome(temperature_index, humidity_index, weirdness);
                let shattered_biome =
                    self.pick_shattered_biome(temperature_index, humidity_index, weirdness);
                let middle_or_savanna = self.maybe_pick_windswept_savanna_biome(
                    temperature_index,
                    humidity_index,
                    weirdness,
                    middle_biome,
                );
                let slope_biome =
                    self.pick_slope_biome(temperature_index, humidity_index, weirdness);
                let peak_biome = self.pick_peak_biome(temperature_index, humidity_index, weirdness);
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    self.erosions[0],
                    weirdness,
                    0.0,
                    slope_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[0],
                    weirdness,
                    0.0,
                    peak_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    self.erosions[1],
                    weirdness,
                    0.0,
                    middle_badlands_or_slope,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[1],
                    weirdness,
                    0.0,
                    slope_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    ClimateParameter::span_parameters(self.erosions[2], self.erosions[3]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[2],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.mid_inland_continentalness,
                    self.erosions[3],
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    self.erosions[3],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[4],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    self.erosions[5],
                    weirdness,
                    0.0,
                    middle_or_savanna,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[5],
                    weirdness,
                    0.0,
                    shattered_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[6],
                    weirdness,
                    0.0,
                    middle_biome,
                );
            }
        }
    }

    fn add_mid_slice(&self, points: &mut Vec<BiomeParameterPoint>, weirdness: ClimateParameter) {
        self.add_surface_biome(
            points,
            self.full_range,
            self.full_range,
            self.coast_continentalness,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[2]),
            weirdness,
            0.0,
            "minecraft:stony_shore",
        );
        self.add_surface_biome(
            points,
            ClimateParameter::span_parameters(self.temperatures[1], self.temperatures[2]),
            self.full_range,
            ClimateParameter::span_parameters(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.add_surface_biome(
            points,
            ClimateParameter::span_parameters(self.temperatures[3], self.temperatures[4]),
            self.full_range,
            ClimateParameter::span_parameters(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );

        for temperature_index in 0..self.temperatures.len() {
            let temperature = self.temperatures[temperature_index];
            for humidity_index in 0..self.humidities.len() {
                let humidity = self.humidities[humidity_index];
                let middle_biome =
                    self.pick_middle_biome(temperature_index, humidity_index, weirdness);
                let middle_or_badlands = self.pick_middle_biome_or_badlands_if_hot(
                    temperature_index,
                    humidity_index,
                    weirdness,
                );
                let middle_badlands_or_slope = self
                    .pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(
                        temperature_index,
                        humidity_index,
                        weirdness,
                    );
                let shattered_biome =
                    self.pick_shattered_biome(temperature_index, humidity_index, weirdness);
                let plateau_biome =
                    self.pick_plateau_biome(temperature_index, humidity_index, weirdness);
                let beach_biome = self.pick_beach_biome(temperature_index);
                let middle_or_savanna = self.maybe_pick_windswept_savanna_biome(
                    temperature_index,
                    humidity_index,
                    weirdness,
                    middle_biome,
                );
                let shattered_coast_biome =
                    self.pick_shattered_coast_biome(temperature_index, humidity_index, weirdness);
                let slope_biome =
                    self.pick_slope_biome(temperature_index, humidity_index, weirdness);
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.near_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[0],
                    weirdness,
                    0.0,
                    slope_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.near_inland_continentalness,
                        self.mid_inland_continentalness,
                    ),
                    self.erosions[1],
                    weirdness,
                    0.0,
                    middle_badlands_or_slope,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    self.erosions[1],
                    weirdness,
                    0.0,
                    if temperature_index == 0 {
                        slope_biome
                    } else {
                        plateau_biome
                    },
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    self.erosions[2],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.mid_inland_continentalness,
                    self.erosions[2],
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    self.erosions[2],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    self.erosions[3],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[3],
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                if weirdness.max < 0 {
                    self.add_surface_biome(
                        points,
                        temperature,
                        humidity,
                        self.coast_continentalness,
                        self.erosions[4],
                        weirdness,
                        0.0,
                        beach_biome,
                    );
                    self.add_surface_biome(
                        points,
                        temperature,
                        humidity,
                        ClimateParameter::span_parameters(
                            self.near_inland_continentalness,
                            self.far_inland_continentalness,
                        ),
                        self.erosions[4],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                } else {
                    self.add_surface_biome(
                        points,
                        temperature,
                        humidity,
                        ClimateParameter::span_parameters(
                            self.coast_continentalness,
                            self.far_inland_continentalness,
                        ),
                        self.erosions[4],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }

                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    self.erosions[5],
                    weirdness,
                    0.0,
                    shattered_coast_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    self.erosions[5],
                    weirdness,
                    0.0,
                    middle_or_savanna,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[5],
                    weirdness,
                    0.0,
                    shattered_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    self.erosions[6],
                    weirdness,
                    0.0,
                    if weirdness.max < 0 {
                        beach_biome
                    } else {
                        middle_biome
                    },
                );

                if temperature_index == 0 {
                    self.add_surface_biome(
                        points,
                        temperature,
                        humidity,
                        ClimateParameter::span_parameters(
                            self.near_inland_continentalness,
                            self.far_inland_continentalness,
                        ),
                        self.erosions[6],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }
            }
        }
    }

    fn add_low_slice(&self, points: &mut Vec<BiomeParameterPoint>, weirdness: ClimateParameter) {
        self.add_surface_biome(
            points,
            self.full_range,
            self.full_range,
            self.coast_continentalness,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[2]),
            weirdness,
            0.0,
            "minecraft:stony_shore",
        );
        self.add_surface_biome(
            points,
            ClimateParameter::span_parameters(self.temperatures[1], self.temperatures[2]),
            self.full_range,
            ClimateParameter::span_parameters(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.add_surface_biome(
            points,
            ClimateParameter::span_parameters(self.temperatures[3], self.temperatures[4]),
            self.full_range,
            ClimateParameter::span_parameters(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );

        for temperature_index in 0..self.temperatures.len() {
            let temperature = self.temperatures[temperature_index];
            for humidity_index in 0..self.humidities.len() {
                let humidity = self.humidities[humidity_index];
                let middle_biome =
                    self.pick_middle_biome(temperature_index, humidity_index, weirdness);
                let middle_or_badlands = self.pick_middle_biome_or_badlands_if_hot(
                    temperature_index,
                    humidity_index,
                    weirdness,
                );
                let middle_badlands_or_slope = self
                    .pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(
                        temperature_index,
                        humidity_index,
                        weirdness,
                    );
                let beach_biome = self.pick_beach_biome(temperature_index);
                let middle_or_savanna = self.maybe_pick_windswept_savanna_biome(
                    temperature_index,
                    humidity_index,
                    weirdness,
                    middle_biome,
                );
                let shattered_coast_biome =
                    self.pick_shattered_coast_biome(temperature_index, humidity_index, weirdness);
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
                    weirdness,
                    0.0,
                    middle_badlands_or_slope,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    ClimateParameter::span_parameters(self.erosions[2], self.erosions[3]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    ClimateParameter::span_parameters(self.erosions[2], self.erosions[3]),
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    ClimateParameter::span_parameters(self.erosions[3], self.erosions[4]),
                    weirdness,
                    0.0,
                    beach_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.near_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[4],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    self.erosions[5],
                    weirdness,
                    0.0,
                    shattered_coast_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    self.erosions[5],
                    weirdness,
                    0.0,
                    middle_or_savanna,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    self.erosions[5],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    self.erosions[6],
                    weirdness,
                    0.0,
                    beach_biome,
                );
                if temperature_index == 0 {
                    self.add_surface_biome(
                        points,
                        temperature,
                        humidity,
                        ClimateParameter::span_parameters(
                            self.near_inland_continentalness,
                            self.far_inland_continentalness,
                        ),
                        self.erosions[6],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }
            }
        }
    }

    fn add_valleys(&self, points: &mut Vec<BiomeParameterPoint>, weirdness: ClimateParameter) {
        self.add_surface_biome(
            points,
            self.frozen_range,
            self.full_range,
            self.coast_continentalness,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
            weirdness,
            0.0,
            if weirdness.max < 0 {
                "minecraft:stony_shore"
            } else {
                "minecraft:frozen_river"
            },
        );
        self.add_surface_biome(
            points,
            self.unfrozen_range,
            self.full_range,
            self.coast_continentalness,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
            weirdness,
            0.0,
            if weirdness.max < 0 {
                "minecraft:stony_shore"
            } else {
                "minecraft:river"
            },
        );
        self.add_surface_biome(
            points,
            self.frozen_range,
            self.full_range,
            self.near_inland_continentalness,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.add_surface_biome(
            points,
            self.unfrozen_range,
            self.full_range,
            self.near_inland_continentalness,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.add_surface_biome(
            points,
            self.frozen_range,
            self.full_range,
            ClimateParameter::span_parameters(
                self.coast_continentalness,
                self.far_inland_continentalness,
            ),
            ClimateParameter::span_parameters(self.erosions[2], self.erosions[5]),
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.add_surface_biome(
            points,
            self.unfrozen_range,
            self.full_range,
            ClimateParameter::span_parameters(
                self.coast_continentalness,
                self.far_inland_continentalness,
            ),
            ClimateParameter::span_parameters(self.erosions[2], self.erosions[5]),
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.add_surface_biome(
            points,
            self.frozen_range,
            self.full_range,
            self.coast_continentalness,
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.add_surface_biome(
            points,
            self.unfrozen_range,
            self.full_range,
            self.coast_continentalness,
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.add_surface_biome(
            points,
            ClimateParameter::span_parameters(self.temperatures[1], self.temperatures[2]),
            self.full_range,
            ClimateParameter::span_parameters(
                self.inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.add_surface_biome(
            points,
            ClimateParameter::span_parameters(self.temperatures[3], self.temperatures[4]),
            self.full_range,
            ClimateParameter::span_parameters(
                self.inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );
        self.add_surface_biome(
            points,
            self.frozen_range,
            self.full_range,
            ClimateParameter::span_parameters(
                self.inland_continentalness,
                self.far_inland_continentalness,
            ),
            self.erosions[6],
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );

        for temperature_index in 0..self.temperatures.len() {
            let temperature = self.temperatures[temperature_index];
            for humidity_index in 0..self.humidities.len() {
                let humidity = self.humidities[humidity_index];
                let middle_or_badlands = self.pick_middle_biome_or_badlands_if_hot(
                    temperature_index,
                    humidity_index,
                    weirdness,
                );
                self.add_surface_biome(
                    points,
                    temperature,
                    humidity,
                    ClimateParameter::span_parameters(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
                    weirdness,
                    0.0,
                    middle_or_badlands,
                );
            }
        }
    }

    fn add_underground_biomes(&self, points: &mut Vec<BiomeParameterPoint>) {
        self.add_underground_biome(
            points,
            self.full_range,
            self.full_range,
            ClimateParameter::span(0.8, 1.0),
            self.full_range,
            self.full_range,
            0.0,
            "minecraft:dripstone_caves",
        );
        self.add_underground_biome(
            points,
            self.full_range,
            ClimateParameter::span(0.7, 1.0),
            self.full_range,
            self.full_range,
            self.full_range,
            0.0,
            "minecraft:lush_caves",
        );
        self.add_bottom_biome(
            points,
            self.full_range,
            self.full_range,
            self.full_range,
            ClimateParameter::span_parameters(self.erosions[0], self.erosions[1]),
            self.full_range,
            0.0,
            "minecraft:deep_dark",
        );
    }

    fn pick_middle_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if weirdness.max < 0 {
            return MIDDLE_BIOMES[temperature_index][humidity_index];
        }

        MIDDLE_BIOMES_VARIANT[temperature_index][humidity_index]
            .unwrap_or(MIDDLE_BIOMES[temperature_index][humidity_index])
    }

    fn pick_middle_biome_or_badlands_if_hot(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if temperature_index == 4 {
            self.pick_badlands_biome(humidity_index, weirdness)
        } else {
            self.pick_middle_biome(temperature_index, humidity_index, weirdness)
        }
    }

    fn pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if temperature_index == 0 {
            self.pick_slope_biome(temperature_index, humidity_index, weirdness)
        } else {
            self.pick_middle_biome_or_badlands_if_hot(temperature_index, humidity_index, weirdness)
        }
    }

    fn maybe_pick_windswept_savanna_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
        underlying_biome: &'static str,
    ) -> &'static str {
        if temperature_index > 1 && humidity_index < 4 && weirdness.max >= 0 {
            "minecraft:windswept_savanna"
        } else {
            underlying_biome
        }
    }

    fn pick_shattered_coast_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        let beach_or_middle = if weirdness.max >= 0 {
            self.pick_middle_biome(temperature_index, humidity_index, weirdness)
        } else {
            self.pick_beach_biome(temperature_index)
        };
        self.maybe_pick_windswept_savanna_biome(
            temperature_index,
            humidity_index,
            weirdness,
            beach_or_middle,
        )
    }

    fn pick_beach_biome(&self, temperature_index: usize) -> &'static str {
        match temperature_index {
            0 => "minecraft:snowy_beach",
            4 => "minecraft:desert",
            _ => "minecraft:beach",
        }
    }

    fn pick_badlands_biome(
        &self,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if humidity_index < 2 {
            if weirdness.max < 0 {
                "minecraft:badlands"
            } else {
                "minecraft:eroded_badlands"
            }
        } else if humidity_index < 3 {
            "minecraft:badlands"
        } else {
            "minecraft:wooded_badlands"
        }
    }

    fn pick_plateau_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if weirdness.max >= 0
            && let Some(variant) = PLATEAU_BIOMES_VARIANT[temperature_index][humidity_index]
        {
            return variant;
        }

        PLATEAU_BIOMES[temperature_index][humidity_index]
    }

    fn pick_peak_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if temperature_index <= 2 {
            if weirdness.max < 0 {
                "minecraft:jagged_peaks"
            } else {
                "minecraft:frozen_peaks"
            }
        } else if temperature_index == 3 {
            "minecraft:stony_peaks"
        } else {
            self.pick_badlands_biome(humidity_index, weirdness)
        }
    }

    fn pick_slope_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        if temperature_index >= 3 {
            self.pick_plateau_biome(temperature_index, humidity_index, weirdness)
        } else if humidity_index <= 1 {
            "minecraft:snowy_slopes"
        } else {
            "minecraft:grove"
        }
    }

    fn pick_shattered_biome(
        &self,
        temperature_index: usize,
        humidity_index: usize,
        weirdness: ClimateParameter,
    ) -> &'static str {
        SHATTERED_BIOMES[temperature_index][humidity_index]
            .unwrap_or_else(|| self.pick_middle_biome(temperature_index, humidity_index, weirdness))
    }

    #[allow(clippy::too_many_arguments)]
    fn add_surface_biome(
        &self,
        points: &mut Vec<BiomeParameterPoint>,
        temperature: ClimateParameter,
        humidity: ClimateParameter,
        continentalness: ClimateParameter,
        erosion: ClimateParameter,
        weirdness: ClimateParameter,
        offset: f64,
        biome: &'static str,
    ) {
        self.push(
            points,
            temperature,
            humidity,
            continentalness,
            erosion,
            ClimateParameter::point(0.0),
            weirdness,
            offset,
            biome,
        );
        self.push(
            points,
            temperature,
            humidity,
            continentalness,
            erosion,
            ClimateParameter::point(1.0),
            weirdness,
            offset,
            biome,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_underground_biome(
        &self,
        points: &mut Vec<BiomeParameterPoint>,
        temperature: ClimateParameter,
        humidity: ClimateParameter,
        continentalness: ClimateParameter,
        erosion: ClimateParameter,
        weirdness: ClimateParameter,
        offset: f64,
        biome: &'static str,
    ) {
        self.push(
            points,
            temperature,
            humidity,
            continentalness,
            erosion,
            ClimateParameter::span(0.2, 0.9),
            weirdness,
            offset,
            biome,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_bottom_biome(
        &self,
        points: &mut Vec<BiomeParameterPoint>,
        temperature: ClimateParameter,
        humidity: ClimateParameter,
        continentalness: ClimateParameter,
        erosion: ClimateParameter,
        weirdness: ClimateParameter,
        offset: f64,
        biome: &'static str,
    ) {
        self.push(
            points,
            temperature,
            humidity,
            continentalness,
            erosion,
            ClimateParameter::point(1.1),
            weirdness,
            offset,
            biome,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &self,
        points: &mut Vec<BiomeParameterPoint>,
        temperature: ClimateParameter,
        humidity: ClimateParameter,
        continentalness: ClimateParameter,
        erosion: ClimateParameter,
        depth: ClimateParameter,
        weirdness: ClimateParameter,
        offset: f64,
        biome: &'static str,
    ) {
        points.push(BiomeParameterPoint {
            parameters: [
                temperature,
                humidity,
                continentalness,
                erosion,
                depth,
                weirdness,
            ],
            offset: quantize_climate(offset),
            biome,
        });
    }
}

const OCEANS: [[&str; 5]; 2] = [
    [
        "minecraft:deep_frozen_ocean",
        "minecraft:deep_cold_ocean",
        "minecraft:deep_ocean",
        "minecraft:deep_lukewarm_ocean",
        "minecraft:warm_ocean",
    ],
    [
        "minecraft:frozen_ocean",
        "minecraft:cold_ocean",
        "minecraft:ocean",
        "minecraft:lukewarm_ocean",
        "minecraft:warm_ocean",
    ],
];

const MIDDLE_BIOMES: [[&str; 5]; 5] = [
    [
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_taiga",
        "minecraft:taiga",
    ],
    [
        "minecraft:plains",
        "minecraft:plains",
        "minecraft:forest",
        "minecraft:taiga",
        "minecraft:old_growth_spruce_taiga",
    ],
    [
        "minecraft:flower_forest",
        "minecraft:plains",
        "minecraft:forest",
        "minecraft:birch_forest",
        "minecraft:dark_forest",
    ],
    [
        "minecraft:savanna",
        "minecraft:savanna",
        "minecraft:forest",
        "minecraft:jungle",
        "minecraft:jungle",
    ],
    [
        "minecraft:desert",
        "minecraft:desert",
        "minecraft:desert",
        "minecraft:desert",
        "minecraft:desert",
    ],
];

const MIDDLE_BIOMES_VARIANT: [[Option<&str>; 5]; 5] = [
    [
        Some("minecraft:ice_spikes"),
        None,
        Some("minecraft:snowy_taiga"),
        None,
        None,
    ],
    [
        None,
        None,
        None,
        None,
        Some("minecraft:old_growth_pine_taiga"),
    ],
    [
        Some("minecraft:sunflower_plains"),
        None,
        None,
        Some("minecraft:old_growth_birch_forest"),
        None,
    ],
    [
        None,
        None,
        Some("minecraft:plains"),
        Some("minecraft:sparse_jungle"),
        Some("minecraft:bamboo_jungle"),
    ],
    [None, None, None, None, None],
];

const PLATEAU_BIOMES: [[&str; 5]; 5] = [
    [
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_plains",
        "minecraft:snowy_taiga",
        "minecraft:snowy_taiga",
    ],
    [
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:forest",
        "minecraft:taiga",
        "minecraft:old_growth_spruce_taiga",
    ],
    [
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:meadow",
        "minecraft:pale_garden",
    ],
    [
        "minecraft:savanna_plateau",
        "minecraft:savanna_plateau",
        "minecraft:forest",
        "minecraft:forest",
        "minecraft:jungle",
    ],
    [
        "minecraft:badlands",
        "minecraft:badlands",
        "minecraft:badlands",
        "minecraft:wooded_badlands",
        "minecraft:wooded_badlands",
    ],
];

const PLATEAU_BIOMES_VARIANT: [[Option<&str>; 5]; 5] = [
    [Some("minecraft:ice_spikes"), None, None, None, None],
    [
        Some("minecraft:cherry_grove"),
        None,
        Some("minecraft:meadow"),
        Some("minecraft:meadow"),
        Some("minecraft:old_growth_pine_taiga"),
    ],
    [
        Some("minecraft:cherry_grove"),
        Some("minecraft:cherry_grove"),
        Some("minecraft:forest"),
        Some("minecraft:birch_forest"),
        None,
    ],
    [None, None, None, None, None],
    [
        Some("minecraft:eroded_badlands"),
        Some("minecraft:eroded_badlands"),
        None,
        None,
        None,
    ],
];

const SHATTERED_BIOMES: [[Option<&str>; 5]; 5] = [
    [
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_forest"),
        Some("minecraft:windswept_forest"),
    ],
    [
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_gravelly_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_forest"),
        Some("minecraft:windswept_forest"),
    ],
    [
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_hills"),
        Some("minecraft:windswept_forest"),
        Some("minecraft:windswept_forest"),
    ],
    [None, None, None, None, None],
    [None, None, None, None, None],
];

fn overworld_biome_points() -> &'static [BiomeParameterPoint] {
    static POINTS: OnceLock<Vec<BiomeParameterPoint>> = OnceLock::new();
    POINTS.get_or_init(|| OverworldBiomeBuilder::new().build())
}

fn quantize_climate(value: f64) -> i64 {
    ((value as f32) * 10000.0) as i64
}

fn is_badlands(biome: &str) -> bool {
    matches!(
        biome,
        "minecraft:badlands" | "minecraft:eroded_badlands" | "minecraft:wooded_badlands"
    )
}

fn has_sandstone_under_surface(biome: &str) -> bool {
    matches!(
        biome,
        "minecraft:warm_ocean" | "minecraft:beach" | "minecraft:snowy_beach" | "minecraft:desert"
    )
}

fn generate_clay_bands(random: &mut XoroshiroRandomSource) -> [SurfaceBlock; 192] {
    let mut clay_bands = [SurfaceBlock::Terracotta; 192];
    let mut index = 0;
    while index < clay_bands.len() {
        index += random.next_int(5) + 1;
        if index < clay_bands.len() {
            clay_bands[index] = SurfaceBlock::OrangeTerracotta;
        }
    }

    make_clay_bands(random, &mut clay_bands, 1, SurfaceBlock::YellowTerracotta);
    make_clay_bands(random, &mut clay_bands, 2, SurfaceBlock::BrownTerracotta);
    make_clay_bands(random, &mut clay_bands, 1, SurfaceBlock::RedTerracotta);

    let white_band_count = 9 + random.next_int(7);
    let mut placed = 0;
    let mut start = 0;
    while placed < white_band_count && start < clay_bands.len() {
        clay_bands[start] = SurfaceBlock::WhiteTerracotta;
        if start > 1 && random.next_bool() {
            clay_bands[start - 1] = SurfaceBlock::LightGrayTerracotta;
        }
        if start + 1 < clay_bands.len() && random.next_bool() {
            clay_bands[start + 1] = SurfaceBlock::LightGrayTerracotta;
        }

        placed += 1;
        start += random.next_int(16) + 4;
    }

    clay_bands
}

fn make_clay_bands(
    random: &mut XoroshiroRandomSource,
    clay_bands: &mut [SurfaceBlock; 192],
    base_width: usize,
    state: SurfaceBlock,
) {
    let band_count = 6 + random.next_int(10);
    for _ in 0..band_count {
        let width = base_width + random.next_int(3);
        let start = random.next_int(clay_bands.len());
        for offset in 0..width {
            if start + offset < clay_bands.len() {
                clay_bands[start + offset] = state;
            }
        }
    }
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

    #[test]
    fn ore_vein_noise_places_seeded_vein_blocks() {
        let veins = OreVeinNoise::new(12345);
        let has_vein = (-32..=32)
            .any(|x| (-32..=32).any(|z| (-60..=50).any(|y| veins.block_at(x, y, z).is_some())));

        assert!(has_vein);
    }

    #[test]
    fn overworld_biome_builder_covers_vanilla_special_biomes() {
        let mushroom_fields = select_overworld_biome(
            &OverworldClimateSample {
                temperature: 0.0,
                vegetation: 0.0,
                continentalness: -1.1,
                erosion: 0.0,
                ridges: 0.0,
                peaks_and_valleys: peaks_and_valleys(0.0),
            },
            0.0,
        );
        let lush_caves = select_overworld_biome(
            &OverworldClimateSample {
                temperature: 0.0,
                vegetation: 0.8,
                continentalness: 0.0,
                erosion: 0.0,
                ridges: 0.0,
                peaks_and_valleys: peaks_and_valleys(0.0),
            },
            0.5,
        );
        let deep_dark = select_overworld_biome(
            &OverworldClimateSample {
                temperature: 0.0,
                vegetation: 0.0,
                continentalness: 0.0,
                erosion: -0.7,
                ridges: 0.0,
                peaks_and_valleys: peaks_and_valleys(0.0),
            },
            1.1,
        );

        assert_eq!(mushroom_fields, "minecraft:mushroom_fields");
        assert_eq!(lush_caves, "minecraft:lush_caves");
        assert_eq!(deep_dark, "minecraft:deep_dark");
    }

    #[test]
    fn overworld_surface_rules_apply_biome_specific_blocks() {
        let rules = OverworldSurfaceRules::new(12345);
        let base = SurfaceRuleContext {
            x: 0,
            y: 80,
            z: 0,
            surface_height: 80,
            above_water: true,
            sea_level: 63,
            min_y: -64,
            biome: "minecraft:plains",
            slope: 0,
        };

        assert_eq!(
            rules.block_at(SurfaceRuleContext {
                biome: "minecraft:mushroom_fields",
                ..base
            }),
            Some(SurfaceBlock::Mycelium)
        );
        assert_eq!(
            rules.block_at(SurfaceRuleContext {
                biome: "minecraft:desert",
                ..base
            }),
            Some(SurfaceBlock::Sand)
        );
        assert_eq!(
            rules.block_at(SurfaceRuleContext {
                biome: "minecraft:frozen_peaks",
                slope: 4,
                ..base
            }),
            Some(SurfaceBlock::PackedIce)
        );
    }

    #[test]
    fn overworld_surface_rules_use_actual_water_height_for_grass() {
        let rules = OverworldSurfaceRules::new(12345);
        let base = SurfaceRuleContext {
            x: 0,
            y: 58,
            z: 0,
            surface_height: 58,
            above_water: true,
            sea_level: 63,
            min_y: -64,
            biome: "minecraft:plains",
            slope: 0,
        };

        assert_eq!(rules.block_at(base), Some(SurfaceBlock::GrassBlock));
        assert_eq!(
            rules.block_at(SurfaceRuleContext {
                above_water: false,
                ..base
            }),
            Some(SurfaceBlock::Dirt)
        );
    }
}
