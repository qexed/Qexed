#[derive(Clone, Copy, Debug)]
enum HeightAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl HeightAnchor {
    fn resolve(self, settings: &NoiseSettings) -> i32 {
        match self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => settings.min_y + offset,
            Self::BelowTop(offset) => settings.min_y + settings.height - 1 - offset,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UniformFloat {
    min: f32,
    max: f32,
}

impl UniformFloat {
    fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    fn sample(self, random: &mut JavaRandom) -> f32 {
        random_between(random, self.min, self.max)
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstantFloat(f32);

impl ConstantFloat {
    fn sample(self, _random: &mut JavaRandom) -> f32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
struct TrapezoidFloat {
    min: f32,
    max: f32,
    plateau: f32,
}

impl TrapezoidFloat {
    fn sample(self, random: &mut JavaRandom) -> f32 {
        let range = self.max - self.min;
        let plateau_start = (range - self.plateau) / 2.0;
        let plateau_end = range - plateau_start;
        self.min + random.next_float() * plateau_end + random.next_float() * plateau_start
    }
}

fn random_between(random: &mut JavaRandom, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

#[derive(Clone, Debug)]
struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 25_214_903_917;
    const ADDEND: u64 = 11;
    const MASK: u64 = (1_u64 << 48) - 1;

    fn new(seed: i64) -> Self {
        let mut random = Self { seed: 0 };
        random.set_seed(seed);
        random
    }

    fn set_seed(&mut self, seed: i64) {
        self.seed = ((seed as u64) ^ Self::MULTIPLIER) & Self::MASK;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND)
            & Self::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if (bound & -bound) == bound {
            return (((bound as i64) * (self.next(31) as i64)) >> 31) as i32;
        }

        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let upper = self.next(32) as i64;
        let lower = self.next(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        ((upper << 27) + lower) as f64 * (1.110_223e-16_f32 as f64)
    }

    fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    fn set_large_feature_seed(&mut self, world_seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(world_seed);
        let x_scale = self.next_long();
        let z_scale = self.next_long();
        let seed = ((chunk_x as i64).wrapping_mul(x_scale))
            ^ ((chunk_z as i64).wrapping_mul(z_scale))
            ^ world_seed;
        self.set_seed(seed);
    }
}
