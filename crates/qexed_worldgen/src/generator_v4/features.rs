use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

const FEATURE_SOURCE_CACHE_LIMIT: usize = 96;
const FEATURE_PROFILE_TOP_COUNT: usize = 12;
const FEATURE_TRACE_ENV: &str = "QEXED_WORLDGEN_FEATURE_TRACE";
const FEATURE_PROFILE_ENV: &str = "QEXED_WORLDGEN_FEATURE_PROFILE";
const FEATURE_TRACE_STOP_AFTER_ENV: &str = "QEXED_WORLDGEN_FEATURE_TRACE_STOP_AFTER";
const FEATURE_TRACE_TIMEOUT_MS_ENV: &str = "QEXED_WORLDGEN_FEATURE_TRACE_TIMEOUT_MS";
const FEATURE_WRITE_TRACE_TARGET_ENV: &str = "QEXED_WORLDGEN_FEATURE_WRITE_TRACE_TARGET";
const FEATURE_WRITE_TRACE_FILTER_ENV: &str = "QEXED_WORLDGEN_FEATURE_WRITE_TRACE_FILTER";
const FULL_DIAGNOSTIC_TEST_NAME: &str = "v4_pipeline_full_chunk_manual_diagnostic";

fn should_profile_to_stderr() -> bool {
    std::env::var_os(FEATURE_PROFILE_ENV).is_some()
        || std::thread::current()
            .name()
            .is_some_and(|name| name.contains(FULL_DIAGNOSTIC_TEST_NAME))
}

#[derive(Debug, Clone, Copy)]
struct FeatureWriteTraceTarget {
    x: i32,
    y: i32,
    z: i32,
}

impl FeatureWriteTraceTarget {
    fn from_env() -> Option<Self> {
        static TARGET: OnceLock<Option<FeatureWriteTraceTarget>> = OnceLock::new();
        *TARGET.get_or_init(|| {
            let value = std::env::var(FEATURE_WRITE_TRACE_TARGET_ENV).ok()?;
            let mut parts = value.split(',').map(str::trim);
            let x = parts.next()?.parse().ok()?;
            let y = parts.next()?.parse().ok()?;
            let z = parts.next()?.parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            Some(Self { x, y, z })
        })
    }

    fn matches(self, x: i32, y: i32, z: i32) -> bool {
        self.x == x && self.y == y && self.z == z
    }

    fn near(self, x: i32, y: i32, z: i32, radius: i32) -> bool {
        (self.x - x).abs() <= radius && (self.y - y).abs() <= radius && (self.z - z).abs() <= radius
    }
}

#[derive(Debug, Clone)]
struct FeatureWriteTraceFilter {
    feature_name: Option<String>,
    step_index: Option<i32>,
    feature_index: Option<i32>,
    phase: Option<String>,
    attempt: Option<i32>,
}

impl FeatureWriteTraceFilter {
    fn from_env() -> Option<Self> {
        static FILTER: OnceLock<Option<FeatureWriteTraceFilter>> = OnceLock::new();
        FILTER
            .get_or_init(|| {
                let value = std::env::var(FEATURE_WRITE_TRACE_FILTER_ENV).ok()?;
                let mut filter = Self {
                    feature_name: None,
                    step_index: None,
                    feature_index: None,
                    phase: None,
                    attempt: None,
                };

                for entry in value
                    .split(',')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                {
                    let Some((key, value)) = entry.split_once('=') else {
                        return None;
                    };
                    match key.trim() {
                        "name" => filter.feature_name = Some(value.trim().to_string()),
                        "step" => filter.step_index = value.trim().parse().ok(),
                        "index" => filter.feature_index = value.trim().parse().ok(),
                        "phase" => filter.phase = Some(value.trim().to_string()),
                        "attempt" => filter.attempt = value.trim().parse().ok(),
                        _ => return None,
                    }
                }

                Some(filter)
            })
            .clone()
    }

    fn matches(&self, context: &FeatureWriteTraceContext) -> bool {
        self.feature_name
            .as_deref()
            .is_none_or(|name| name == context.feature_name)
            && self
                .step_index
                .is_none_or(|step_index| step_index == context.step_index)
            && self
                .feature_index
                .is_none_or(|feature_index| feature_index == context.feature_index)
            && self
                .phase
                .as_deref()
                .is_none_or(|phase| phase == context.phase)
            && self
                .attempt
                .is_none_or(|attempt| context.attempt == Some(attempt))
    }
}

#[derive(Debug, Clone)]
struct FeatureWriteTraceContext {
    chunk_x: i32,
    chunk_z: i32,
    ordinal: usize,
    feature_name: &'static str,
    step_index: i32,
    feature_index: i32,
    phase: &'static str,
    source_chunk_x: i32,
    source_chunk_z: i32,
    source_origin_x: i32,
    source_origin_z: i32,
    target_chunk_x: i32,
    target_chunk_z: i32,
    target_origin_x: i32,
    target_origin_z: i32,
    decoration_seed: i64,
    attempt: Option<i32>,
    attempt_origin: Option<(i32, i32, i32)>,
}

thread_local! {
    static FEATURE_WRITE_TRACE_CONTEXT: std::cell::RefCell<Option<FeatureWriteTraceContext>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Debug, Clone, Default)]
struct OrePlacementCountTrace {
    target_predicate_matches: usize,
    should_skip_air_check_next_float_calls: usize,
    set_block_state_writes: usize,
}

thread_local! {
    static ORE_PLACEMENT_COUNT_TRACE: std::cell::RefCell<Option<OrePlacementCountTrace>> =
        const { std::cell::RefCell::new(None) };
}

fn with_feature_write_trace_context<T>(
    context: FeatureWriteTraceContext,
    action: impl FnOnce() -> T,
) -> T {
    FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
        let previous = current.replace(Some(context));
        let result = action();
        current.replace(previous);
        result
    })
}

fn update_feature_write_trace_attempt(attempt: i32, origin_x: i32, origin_y: i32, origin_z: i32) {
    FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
        if let Some(context) = current.borrow_mut().as_mut() {
            context.attempt = Some(attempt);
            context.attempt_origin = Some((origin_x, origin_y, origin_z));
        }
    });
}

fn trace_feature_write_at_target(
    world_x: i32,
    world_y: i32,
    world_z: i32,
    chunk_origin_x: i32,
    chunk_origin_z: i32,
    local_x: usize,
    local_z: usize,
    previous: &BlockLayer,
    replacement: &BlockLayer,
) {
    if !FeatureWriteTraceTarget::from_env()
        .is_some_and(|target| target.matches(world_x, world_y, world_z))
    {
        return;
    }

    FEATURE_WRITE_TRACE_CONTEXT.with(|current| {
        if let Some(context) = current.borrow().as_ref() {
            if FeatureWriteTraceFilter::from_env().is_some_and(|filter| !filter.matches(context)) {
                return;
            }
            eprintln!(
                "feature write trace: chunk=({},{}) coord=({world_x},{world_y},{world_z}) local=({local_x},{world_y},{local_z}) phase={} ordinal={} name={} step={} index={} source_chunk=({},{}) source_origin=({},{}) target_chunk=({},{}) target_origin=({},{}) write_origin=({chunk_origin_x},{chunk_origin_z}) decoration_seed={} attempt={} attempt_origin={} previous={} replacement={}",
                context.chunk_x,
                context.chunk_z,
                context.phase,
                context.ordinal,
                context.feature_name,
                context.step_index,
                context.feature_index,
                context.source_chunk_x,
                context.source_chunk_z,
                context.source_origin_x,
                context.source_origin_z,
                context.target_chunk_x,
                context.target_chunk_z,
                context.target_origin_x,
                context.target_origin_z,
                context.decoration_seed,
                context
                    .attempt
                    .map(|attempt| attempt.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                context
                    .attempt_origin
                    .map(|(x, y, z)| format!("({x},{y},{z})"))
                    .unwrap_or_else(|| "none".to_string()),
                previous.block,
                replacement.block
            );
        }
    });
}

#[derive(Debug, Clone)]
struct WorldgenStageTrace {
    enabled: bool,
    chunk_x: i32,
    chunk_z: i32,
}

impl WorldgenStageTrace {
    fn from_env(chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            enabled: std::env::var_os(FEATURE_TRACE_ENV).is_some(),
            chunk_x,
            chunk_z,
        }
    }

    fn start(&self, stage: &str) {
        if self.enabled {
            eprintln!(
                "worldgen stage trace start: chunk=({},{}) stage={stage}",
                self.chunk_x, self.chunk_z
            );
        }
    }

    fn done(&self, stage: &str, elapsed: Duration) {
        if self.enabled {
            eprintln!(
                "worldgen stage trace done: chunk=({},{}) stage={stage} elapsed_ms={:.2}",
                self.chunk_x,
                self.chunk_z,
                duration_ms(elapsed)
            );
        }
    }
}

#[derive(Debug, Clone)]
struct FeatureTrace {
    enabled: bool,
    stop_after: Option<FeatureTraceStopAfter>,
    timeout: Option<Duration>,
    started: Instant,
}

impl FeatureTrace {
    fn from_env() -> Self {
        Self {
            enabled: std::env::var_os(FEATURE_TRACE_ENV).is_some(),
            stop_after: std::env::var(FEATURE_TRACE_STOP_AFTER_ENV)
                .ok()
                .and_then(|value| FeatureTraceStopAfter::parse(&value)),
            timeout: std::env::var(FEATURE_TRACE_TIMEOUT_MS_ENV)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .map(Duration::from_millis),
            started: Instant::now(),
        }
    }

    fn start(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        ordinal: usize,
        feature_name: &str,
        step_index: i32,
        feature_index: i32,
    ) -> Option<Instant> {
        if !self.enabled {
            return None;
        }
        eprintln!(
            "feature trace start: chunk=({chunk_x},{chunk_z}) ordinal={ordinal} name={feature_name} step={step_index} index={feature_index}"
        );
        Some(Instant::now())
    }

    fn done(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        ordinal: usize,
        feature_name: &str,
        step_index: i32,
        feature_index: i32,
        start: Instant,
    ) {
        eprintln!(
            "feature trace done: chunk=({chunk_x},{chunk_z}) ordinal={ordinal} name={feature_name} step={step_index} index={feature_index} elapsed_ms={:.2}",
            duration_ms(start.elapsed())
        );
    }

    fn should_stop(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        ordinal: usize,
        feature_name: &str,
        step_index: i32,
        feature_index: i32,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        let elapsed = self.started.elapsed();
        let timed_out = self.timeout.is_some_and(|timeout| elapsed >= timeout);
        let stop_after = self.stop_after.as_ref().is_some_and(|stop_after| {
            stop_after.matches(ordinal, feature_name, step_index, feature_index)
        });
        if timed_out || stop_after {
            eprintln!(
                "feature trace stop: chunk=({chunk_x},{chunk_z}) reason={} ordinal={ordinal} name={feature_name} step={step_index} index={feature_index} elapsed_ms={:.2}",
                if timed_out { "timeout" } else { "stop_after" },
                duration_ms(elapsed)
            );
            return true;
        }
        false
    }
}

#[derive(Debug, Clone)]
enum FeatureTraceStopAfter {
    Name(String),
    Ordinal(usize),
    StepIndex { step: i32, index: i32 },
}

impl FeatureTraceStopAfter {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        if let Some((step, index)) = value.split_once(':') {
            return Some(Self::StepIndex {
                step: step.trim().parse().ok()?,
                index: index.trim().parse().ok()?,
            });
        }
        if let Some(ordinal) = value
            .strip_prefix('#')
            .and_then(|value| value.trim().parse().ok())
        {
            return Some(Self::Ordinal(ordinal));
        }
        Some(Self::Name(value.to_string()))
    }

    fn matches(
        &self,
        ordinal: usize,
        feature_name: &str,
        step_index: i32,
        feature_index: i32,
    ) -> bool {
        match self {
            Self::Name(name) => name == feature_name,
            Self::Ordinal(stop_ordinal) => *stop_ordinal == ordinal,
            Self::StepIndex { step, index } => *step == step_index && *index == feature_index,
        }
    }
}

#[derive(Debug, Clone)]
struct OverworldOreFeatures {
    seed: i64,
    ordered_feature_keys: Vec<PlacedUndergroundFeatureKey>,
    features: Vec<PlacedOreFeature>,
    underwater_magma: PlacedUnderwaterMagmaFeature,
    lush_clay_ore: PlacedOreFeature,
    disks: Vec<PlacedDiskFeature>,
    springs: Vec<PlacedSpringFeature>,
    lakes: Vec<PlacedLakeFeature>,
    geodes: Vec<PlacedGeodeFeature>,
    dripstone_features: Vec<PlacedDripstoneFeature>,
    sculk_features: Vec<PlacedSculkFeature>,
    structure_features: Vec<PlacedStructureFeature>,
    surface_features: Vec<PlacedSurfaceFeature>,
    monster_rooms: Vec<PlacedMonsterRoomFeature>,
    glow_lichen: PlacedMultifaceGrowthFeature,
    cave_vines: PlacedCaveVinesFeature,
    classic_vines: PlacedClassicVinesFeature,
    spore_blossom: PlacedSporeBlossomFeature,
    environment_scan_features: Vec<PlacedEnvironmentScanFeature>,
    aquatic_features: Vec<PlacedAquaticFeature>,
    huge_mushrooms: Vec<PlacedHugeMushroomFeature>,
    vegetation_patches: Vec<PlacedSimpleVegetationFeature>,
    surface_vines: PlacedClassicVinesFeature,
    block_columns: Vec<PlacedBlockColumnFeature>,
    trees: Vec<PlacedTreeFeature>,
    freeze_top_layer: PlacedFreezeTopLayerFeature,
}

impl OverworldOreFeatures {
    fn new(seed: i64) -> Self {
        let dirt = OreFeatureConfig::base_stone(33, "minecraft:dirt");
        let gravel = OreFeatureConfig::base_stone(33, "minecraft:gravel");
        let clay_ore = OreFeatureConfig::base_stone(33, "minecraft:clay");
        let granite = OreFeatureConfig::base_stone(64, "minecraft:granite");
        let diorite = OreFeatureConfig::base_stone(64, "minecraft:diorite");
        let andesite = OreFeatureConfig::base_stone(64, "minecraft:andesite");
        let tuff = OreFeatureConfig::base_stone(64, "minecraft:tuff");
        let coal = OreFeatureConfig::new(
            17,
            0.0,
            "minecraft:coal_ore",
            "minecraft:deepslate_coal_ore",
        );
        let coal_buried = OreFeatureConfig::new(
            17,
            0.5,
            "minecraft:coal_ore",
            "minecraft:deepslate_coal_ore",
        );
        let iron =
            OreFeatureConfig::new(9, 0.0, "minecraft:iron_ore", "minecraft:deepslate_iron_ore");
        let iron_small =
            OreFeatureConfig::new(4, 0.0, "minecraft:iron_ore", "minecraft:deepslate_iron_ore");
        let gold =
            OreFeatureConfig::new(9, 0.5, "minecraft:gold_ore", "minecraft:deepslate_gold_ore");
        let redstone = OreFeatureConfig::new(
            8,
            0.0,
            "minecraft:redstone_ore",
            "minecraft:deepslate_redstone_ore",
        );
        let diamond_small = OreFeatureConfig::new(
            4,
            0.5,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_medium = OreFeatureConfig::new(
            8,
            0.5,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_large = OreFeatureConfig::new(
            12,
            0.7,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let diamond_buried = OreFeatureConfig::new(
            8,
            1.0,
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        );
        let lapis = OreFeatureConfig::new(
            7,
            0.0,
            "minecraft:lapis_ore",
            "minecraft:deepslate_lapis_ore",
        );
        let lapis_buried = OreFeatureConfig::new(
            7,
            1.0,
            "minecraft:lapis_ore",
            "minecraft:deepslate_lapis_ore",
        );
        let copper = OreFeatureConfig::new(
            10,
            0.0,
            "minecraft:copper_ore",
            "minecraft:deepslate_copper_ore",
        );
        let copper_large = OreFeatureConfig::new(
            20,
            0.0,
            "minecraft:copper_ore",
            "minecraft:deepslate_copper_ore",
        );
        let emerald = OreFeatureConfig::new(
            3,
            0.0,
            "minecraft:emerald_ore",
            "minecraft:deepslate_emerald_ore",
        );
        let infested = OreFeatureConfig::new(
            9,
            0.0,
            "minecraft:infested_stone",
            "minecraft:infested_deepslate",
        );

        let mut features = Self {
            seed,
            ordered_feature_keys: Vec::new(),
            features: vec![
                PlacedOreFeature::new(
                    0,
                    OrePlacementCount::Constant(7),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(160)),
                    dirt,
                ),
                PlacedOreFeature::new(
                    1,
                    OrePlacementCount::Constant(14),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::BelowTop(0)),
                    gravel,
                ),
                PlacedOreFeature::new(
                    2,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    granite.clone(),
                ),
                PlacedOreFeature::new(
                    3,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    granite,
                ),
                PlacedOreFeature::new(
                    4,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    diorite.clone(),
                ),
                PlacedOreFeature::new(
                    5,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    diorite,
                ),
                PlacedOreFeature::new(
                    6,
                    OrePlacementCount::Rarity(6),
                    OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(128)),
                    andesite.clone(),
                ),
                PlacedOreFeature::new(
                    7,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::Absolute(60)),
                    andesite,
                ),
                PlacedOreFeature::new(
                    8,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(0)),
                    tuff,
                ),
                PlacedOreFeature::new(
                    9,
                    OrePlacementCount::Constant(30),
                    OreHeight::Uniform(HeightAnchor::Absolute(136), HeightAnchor::BelowTop(0)),
                    coal,
                ),
                PlacedOreFeature::new(
                    10,
                    OrePlacementCount::Constant(20),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(0), HeightAnchor::Absolute(192)),
                    coal_buried,
                ),
                PlacedOreFeature::new(
                    11,
                    OrePlacementCount::Constant(90),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(80), HeightAnchor::Absolute(384)),
                    iron.clone(),
                ),
                PlacedOreFeature::new(
                    12,
                    OrePlacementCount::Constant(10),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-24), HeightAnchor::Absolute(56)),
                    iron,
                ),
                PlacedOreFeature::new(
                    13,
                    OrePlacementCount::Constant(10),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(72)),
                    iron_small,
                ),
                PlacedOreFeature::new(
                    14,
                    OrePlacementCount::Constant(4),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(32)),
                    gold.clone(),
                ),
                PlacedOreFeature::new(
                    15,
                    OrePlacementCount::Uniform { min: 0, max: 1 },
                    OreHeight::Uniform(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(-48)),
                    gold.clone(),
                ),
                PlacedOreFeature::new(
                    16,
                    OrePlacementCount::Constant(4),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(15)),
                    redstone.clone(),
                ),
                PlacedOreFeature::new(
                    17,
                    OrePlacementCount::Constant(8),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-32),
                        HeightAnchor::AboveBottom(32),
                    ),
                    redstone,
                ),
                PlacedOreFeature::new(
                    18,
                    OrePlacementCount::Constant(7),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_small,
                ),
                PlacedOreFeature::new(
                    19,
                    OrePlacementCount::Constant(2),
                    OreHeight::Uniform(HeightAnchor::Absolute(-64), HeightAnchor::Absolute(-4)),
                    diamond_medium,
                ),
                PlacedOreFeature::new(
                    20,
                    OrePlacementCount::Rarity(9),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_large,
                ),
                PlacedOreFeature::new(
                    21,
                    OrePlacementCount::Constant(4),
                    OreHeight::Trapezoid(
                        HeightAnchor::AboveBottom(-80),
                        HeightAnchor::AboveBottom(80),
                    ),
                    diamond_buried,
                ),
                PlacedOreFeature::new(
                    22,
                    OrePlacementCount::Constant(2),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-32), HeightAnchor::Absolute(32)),
                    lapis,
                ),
                PlacedOreFeature::new(
                    23,
                    OrePlacementCount::Constant(4),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(64)),
                    lapis_buried,
                ),
                PlacedOreFeature::new(
                    24,
                    OrePlacementCount::Constant(16),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(112)),
                    copper_large,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(DRIPSTONE_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    24,
                    OrePlacementCount::Constant(16),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(112)),
                    copper,
                )
                .with_biome_filter(FeatureBiomeFilter::Exclude(DRIPSTONE_CAVES_ORE_BIOMES)),
                PlacedOreFeature::new(
                    29,
                    OrePlacementCount::Constant(50),
                    OreHeight::Uniform(HeightAnchor::Absolute(32), HeightAnchor::Absolute(256)),
                    gold,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_ORE_BIOMES)),
                PlacedOreFeature::new(
                    29,
                    OrePlacementCount::Constant(100),
                    OreHeight::Trapezoid(HeightAnchor::Absolute(-16), HeightAnchor::Absolute(480)),
                    emerald,
                )
                .with_biome_filter(FeatureBiomeFilter::Include(EMERALD_ORE_BIOMES)),
                PlacedOreFeature::new(
                    0,
                    OrePlacementCount::Constant(14),
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(63)),
                    infested,
                )
                .with_step_index(7)
                .with_biome_filter(FeatureBiomeFilter::Include(EMERALD_ORE_BIOMES)),
            ],
            underwater_magma: PlacedUnderwaterMagmaFeature::new(25),
            lush_clay_ore: PlacedOreFeature::new(
                28,
                OrePlacementCount::Constant(46),
                OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256)),
                clay_ore,
            )
            .with_biome_filter(FeatureBiomeFilter::Include(LUSH_CAVES_ORE_BIOMES)),
            disks: vec![
                PlacedDiskFeature::sand(30)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_OR_LUSH_ORE_BIOMES)),
                PlacedDiskFeature::sand(30)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_OR_LUSH_ORE_BIOMES)),
                PlacedDiskFeature::clay(31)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_OR_LUSH_ORE_BIOMES)),
                PlacedDiskFeature::clay(31)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_OR_LUSH_ORE_BIOMES)),
                PlacedDiskFeature::gravel(32)
                    .with_biome_filter(FeatureBiomeFilter::Exclude(BADLANDS_OR_LUSH_ORE_BIOMES)),
                PlacedDiskFeature::gravel(32)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_OR_LUSH_ORE_BIOMES)),
                PlacedDiskFeature::grass(27)
                    .with_surface_anchor("minecraft:mud", -1)
                    .with_biome_filter(FeatureBiomeFilter::Include(MANGROVE_TREE_BIOMES)),
            ],
            springs: vec![
                PlacedSpringFeature::water(0),
                PlacedSpringFeature::lava_overworld(1),
                PlacedSpringFeature::lava_frozen(2),
            ],
            lakes: vec![
                PlacedLakeFeature::lava_underground(0),
                PlacedLakeFeature::lava_surface(1),
            ],
            geodes: vec![PlacedGeodeFeature::amethyst(0)],
            dripstone_features: vec![
                PlacedDripstoneFeature::large(1),
                PlacedDripstoneFeature::cluster(0),
                PlacedDripstoneFeature::pointed(1),
            ],
            sculk_features: vec![
                PlacedSculkFeature::vein(0),
                PlacedSculkFeature::deep_dark_patch(1),
            ],
            structure_features: vec![
                PlacedStructureFeature::fossil_upper(2),
                PlacedStructureFeature::fossil_lower(3),
                PlacedStructureFeature::desert_well(0),
            ],
            surface_features: vec![
                PlacedSurfaceFeature::forest_rock(1),
                PlacedSurfaceFeature::iceberg_packed(2),
                PlacedSurfaceFeature::iceberg_blue(3),
                PlacedSurfaceFeature::ice_spike(0),
                PlacedSurfaceFeature::ice_patch(1),
                PlacedSurfaceFeature::blue_ice(4),
                PlacedSurfaceFeature::pale_moss_patch(90),
            ],
            monster_rooms: vec![
                PlacedMonsterRoomFeature::regular(0),
                PlacedMonsterRoomFeature::deep(1),
            ],
            glow_lichen: PlacedMultifaceGrowthFeature::glow_lichen(0),
            cave_vines: PlacedCaveVinesFeature::new(77),
            classic_vines: PlacedClassicVinesFeature::cave(83),
            spore_blossom: PlacedSporeBlossomFeature::new(78),
            environment_scan_features: vec![
                PlacedEnvironmentScanFeature::lush_caves_ceiling_vegetation(79),
                PlacedEnvironmentScanFeature::lush_caves_clay(80),
                PlacedEnvironmentScanFeature::lush_caves_vegetation(81),
                PlacedEnvironmentScanFeature::rooted_azalea_tree(82),
            ],
            aquatic_features: vec![
                PlacedAquaticFeature::seagrass(66, 48, 0.3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_NORMAL_BIOMES)),
                PlacedAquaticFeature::seagrass(67, 32, 0.3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_COLD_BIOMES)),
                PlacedAquaticFeature::seagrass(68, 48, 0.8)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_DEEP_BIOMES)),
                PlacedAquaticFeature::seagrass(69, 48, 0.8)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_DEEP_COLD_BIOMES)),
                PlacedAquaticFeature::seagrass(70, 48, 0.8)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_DEEP_WARM_BIOMES)),
                PlacedAquaticFeature::seagrass(71, 80, 0.3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_WARM_BIOMES)),
                PlacedAquaticFeature::seagrass(72, 64, 0.6)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_SWAMP_BIOMES)),
                PlacedAquaticFeature::seagrass(73, 48, 0.4)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEAGRASS_RIVER_BIOMES)),
                PlacedAquaticFeature::kelp(74, 120)
                    .with_biome_filter(FeatureBiomeFilter::Include(KELP_COLD_BIOMES)),
                PlacedAquaticFeature::kelp(75, 80)
                    .with_biome_filter(FeatureBiomeFilter::Include(KELP_WARM_BIOMES)),
                PlacedAquaticFeature::sea_pickle(76)
                    .with_biome_filter(FeatureBiomeFilter::Include(SEA_PICKLE_BIOMES)),
                PlacedAquaticFeature::warm_ocean_vegetation(102)
                    .with_biome_filter(FeatureBiomeFilter::Include(WARM_OCEAN_VEGETATION_BIOMES)),
            ],
            huge_mushrooms: vec![PlacedHugeMushroomFeature::mushroom_island_vegetation(101)],
            vegetation_patches: vec![
                PlacedSimpleVegetationFeature::patch_tall_grass_2(1)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_TALL_GRASS_2_BIOMES)),
                PlacedSimpleVegetationFeature::patch_tall_grass(0)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_TALL_GRASS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_bush(2)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_BUSH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_sunflower(3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUNFLOWER_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::flower_plains(4)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_PLAINS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_plain(5)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_PLAIN_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_normal(6)
                    .with_biome_filter(FeatureBiomeFilter::Include(NORMAL_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_normal(7)
                    .with_biome_filter(FeatureBiomeFilter::Include(NORMAL_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::patch_pumpkin(8)
                    .with_biome_filter(FeatureBiomeFilter::Include(PUMPKIN_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(9, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_NORMAL_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(10, 2)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_DESERT_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dead_bush(11, 20)
                    .with_biome_filter(FeatureBiomeFilter::Include(DEAD_BUSH_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_melon(12, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(MELON_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_melon(13, 64)
                    .with_biome_filter(FeatureBiomeFilter::Include(MELON_SPARSE_PATCH_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_normal(20)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_NORMAL_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_forest(21)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_FOREST_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_badlands(22)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_savanna(23)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_SAVANNA_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_taiga(24)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_TAIGA_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_taiga_2(25)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_TAIGA_2_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_jungle(26)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_JUNGLE_BIOMES)),
                PlacedSimpleVegetationFeature::patch_grass_meadow(27)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_GRASS_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::patch_large_fern(28)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_LARGE_FERN_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dry_grass(29, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(DRY_GRASS_DESERT_BIOMES)),
                PlacedSimpleVegetationFeature::patch_dry_grass(30, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(DRY_GRASS_BADLANDS_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_taiga(31)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_taiga(32)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_old_growth(33)
                    .with_biome_filter(FeatureBiomeFilter::Include(OLD_GROWTH_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_old_growth(34)
                    .with_biome_filter(FeatureBiomeFilter::Include(OLD_GROWTH_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::brown_mushroom_swamp(35)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::red_mushroom_swamp(36)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_MUSHROOM_BIOMES)),
                PlacedSimpleVegetationFeature::flower_default(37)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_DEFAULT_BIOMES)),
                PlacedSimpleVegetationFeature::flower_warm(38)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_WARM_BIOMES)),
                PlacedSimpleVegetationFeature::flower_swamp(39)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_SWAMP_BIOMES)),
                PlacedSimpleVegetationFeature::flower_cherry(40)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_CHERRY_BIOMES)),
                PlacedSimpleVegetationFeature::flower_pale_garden(41)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_PALE_GARDEN_BIOMES)),
                PlacedSimpleVegetationFeature::flower_meadow(91)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::flower_flower_forest(92)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_FLOWER_FOREST_BIOMES)),
                PlacedSimpleVegetationFeature::forest_flowers(93, -3, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(FOREST_FLOWERS_BIOMES)),
                PlacedSimpleVegetationFeature::forest_flowers(94, -1, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_FOREST_FLOWERS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_leaf_litter(95)
                    .with_biome_filter(FeatureBiomeFilter::Include(PATCH_LEAF_LITTER_BIOMES)),
                PlacedSimpleVegetationFeature::wildflowers_meadow(96)
                    .with_biome_filter(FeatureBiomeFilter::Include(WILDFLOWERS_MEADOW_BIOMES)),
                PlacedSimpleVegetationFeature::wildflowers_birch_forest(97).with_biome_filter(
                    FeatureBiomeFilter::Include(WILDFLOWERS_BIRCH_FOREST_BIOMES),
                ),
                PlacedSimpleVegetationFeature::pale_garden_flowers(98)
                    .with_biome_filter(FeatureBiomeFilter::Include(PALE_GARDEN_FLOWERS_BIOMES)),
                PlacedSimpleVegetationFeature::patch_berry_common(85)
                    .with_biome_filter(FeatureBiomeFilter::Include(BERRY_COMMON_BIOMES)),
                PlacedSimpleVegetationFeature::patch_berry_rare(86)
                    .with_biome_filter(FeatureBiomeFilter::Include(BERRY_RARE_BIOMES)),
                PlacedSimpleVegetationFeature::patch_firefly_bush_swamp(87)
                    .with_biome_filter(FeatureBiomeFilter::Include(FIREFLY_BUSH_SWAMP_BIOMES)),
                PlacedSimpleVegetationFeature::patch_firefly_bush_near_water(88, 2)
                    .with_biome_filter(FeatureBiomeFilter::Include(FIREFLY_BUSH_NEAR_WATER_BIOMES)),
                PlacedSimpleVegetationFeature::patch_firefly_bush_near_water(89, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(FIREFLY_BUSH_SWAMP_BIOMES)),
                PlacedSimpleVegetationFeature::patch_waterlily(84)
                    .with_biome_filter(FeatureBiomeFilter::Include(WATERLILY_BIOMES)),
            ],
            surface_vines: PlacedClassicVinesFeature::surface(85),
            block_columns: vec![
                PlacedBlockColumnFeature::sugar_cane(14, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_NORMAL_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(15, 5)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_BADLANDS_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(16, 1)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_DESERT_BIOMES)),
                PlacedBlockColumnFeature::sugar_cane(17, 3)
                    .with_biome_filter(FeatureBiomeFilter::Include(SUGAR_CANE_SWAMP_BIOMES)),
                PlacedBlockColumnFeature::cactus(18, 6)
                    .with_biome_filter(FeatureBiomeFilter::Include(CACTUS_DESERT_BIOMES)),
                PlacedBlockColumnFeature::cactus(19, 13)
                    .with_biome_filter(FeatureBiomeFilter::Include(CACTUS_DECORATED_BIOMES)),
                PlacedBlockColumnFeature::bamboo_light(99)
                    .with_biome_filter(FeatureBiomeFilter::Include(BAMBOO_LIGHT_BIOMES)),
                PlacedBlockColumnFeature::bamboo_some_podzol(100)
                    .with_biome_filter(FeatureBiomeFilter::Include(BAMBOO_SOME_PODZOL_BIOMES)),
            ],
            trees: vec![
                PlacedTreeFeature::trees_plains(3)
                    .with_biome_filter(FeatureBiomeFilter::Include(PLAINS_TREE_BIOMES)),
                PlacedTreeFeature::trees_taiga(44)
                    .with_biome_filter(FeatureBiomeFilter::Include(TAIGA_TREE_BIOMES)),
                PlacedTreeFeature::trees_snowy(45)
                    .with_biome_filter(FeatureBiomeFilter::Include(SNOWY_TREE_BIOMES)),
                PlacedTreeFeature::trees_savanna(46)
                    .with_biome_filter(FeatureBiomeFilter::Include(SAVANNA_TREE_BIOMES)),
                PlacedTreeFeature::trees_windswept_savanna(47)
                    .with_biome_filter(FeatureBiomeFilter::Include(WINDSWEPT_SAVANNA_TREE_BIOMES)),
                PlacedTreeFeature::trees_birch(42)
                    .with_biome_filter(FeatureBiomeFilter::Include(BIRCH_TREE_BIOMES)),
                PlacedTreeFeature::trees_tall_birch(43)
                    .with_biome_filter(FeatureBiomeFilter::Include(TALL_BIRCH_TREE_BIOMES)),
                PlacedTreeFeature::dark_forest_vegetation(48)
                    .with_biome_filter(FeatureBiomeFilter::Include(DARK_FOREST_TREE_BIOMES)),
                PlacedTreeFeature::pale_garden_vegetation(49)
                    .with_biome_filter(FeatureBiomeFilter::Include(PALE_GARDEN_TREE_BIOMES)),
                PlacedTreeFeature::trees_flower_forest(50)
                    .with_biome_filter(FeatureBiomeFilter::Include(FLOWER_FOREST_TREE_BIOMES)),
                PlacedTreeFeature::trees_meadow(51)
                    .with_biome_filter(FeatureBiomeFilter::Include(MEADOW_TREE_BIOMES)),
                PlacedTreeFeature::trees_cherry(52)
                    .with_biome_filter(FeatureBiomeFilter::Include(CHERRY_TREE_BIOMES)),
                PlacedTreeFeature::trees_grove(53)
                    .with_biome_filter(FeatureBiomeFilter::Include(GROVE_TREE_BIOMES)),
                PlacedTreeFeature::trees_badlands(54)
                    .with_biome_filter(FeatureBiomeFilter::Include(BADLANDS_TREE_BIOMES)),
                PlacedTreeFeature::trees_swamp(55)
                    .with_biome_filter(FeatureBiomeFilter::Include(SWAMP_TREE_BIOMES)),
                PlacedTreeFeature::trees_windswept_hills(56)
                    .with_biome_filter(FeatureBiomeFilter::Include(WINDSWEPT_HILLS_TREE_BIOMES)),
                PlacedTreeFeature::trees_windswept_forest(57)
                    .with_biome_filter(FeatureBiomeFilter::Include(WINDSWEPT_FOREST_TREE_BIOMES)),
                PlacedTreeFeature::trees_water(58)
                    .with_biome_filter(FeatureBiomeFilter::Include(WATER_TREE_BIOMES)),
                PlacedTreeFeature::trees_birch_and_oak_leaf_litter(59).with_biome_filter(
                    FeatureBiomeFilter::Include(BIRCH_AND_OAK_LEAF_LITTER_TREE_BIOMES),
                ),
                PlacedTreeFeature::trees_sparse_jungle(60)
                    .with_biome_filter(FeatureBiomeFilter::Include(SPARSE_JUNGLE_TREE_BIOMES)),
                PlacedTreeFeature::trees_old_growth_spruce_taiga(61).with_biome_filter(
                    FeatureBiomeFilter::Include(OLD_GROWTH_SPRUCE_TAIGA_TREE_BIOMES),
                ),
                PlacedTreeFeature::trees_old_growth_pine_taiga(62).with_biome_filter(
                    FeatureBiomeFilter::Include(OLD_GROWTH_PINE_TAIGA_TREE_BIOMES),
                ),
                PlacedTreeFeature::trees_jungle(63)
                    .with_biome_filter(FeatureBiomeFilter::Include(JUNGLE_TREE_BIOMES)),
                PlacedTreeFeature::bamboo_vegetation(64)
                    .with_biome_filter(FeatureBiomeFilter::Include(BAMBOO_JUNGLE_TREE_BIOMES)),
                PlacedTreeFeature::trees_mangrove(65)
                    .with_biome_filter(FeatureBiomeFilter::Include(MANGROVE_TREE_BIOMES)),
            ],
            freeze_top_layer: PlacedFreezeTopLayerFeature::new(0),
        };
        features.ordered_feature_keys = features.build_ordered_feature_keys();
        features
    }

    fn build_ordered_feature_keys(&self) -> Vec<PlacedUndergroundFeatureKey> {
        let mut features = Vec::with_capacity(
            self.features.len()
                + self.disks.len()
                + self.springs.len()
                + self.lakes.len()
                + self.geodes.len()
                + self.dripstone_features.len()
                + self.sculk_features.len()
                + self.structure_features.len()
                + self.surface_features.len()
                + self.monster_rooms.len()
                + self.environment_scan_features.len()
                + self.aquatic_features.len()
                + self.huge_mushrooms.len()
                + self.vegetation_patches.len()
                + self.block_columns.len()
                + self.trees.len()
                + 6,
        );
        features.extend((0..self.lakes.len()).map(PlacedUndergroundFeatureKey::Lake));
        features.extend((0..self.geodes.len()).map(PlacedUndergroundFeatureKey::Geode));
        features
            .extend((0..self.dripstone_features.len()).map(PlacedUndergroundFeatureKey::Dripstone));
        features.extend((0..self.sculk_features.len()).map(PlacedUndergroundFeatureKey::Sculk));
        features
            .extend((0..self.structure_features.len()).map(PlacedUndergroundFeatureKey::Structure));
        features.extend((0..self.surface_features.len()).map(PlacedUndergroundFeatureKey::Surface));
        features
            .extend((0..self.monster_rooms.len()).map(PlacedUndergroundFeatureKey::MonsterRoom));
        features.extend((0..self.features.len()).map(PlacedUndergroundFeatureKey::Ore));
        features.push(PlacedUndergroundFeatureKey::UnderwaterMagma);
        features.push(PlacedUndergroundFeatureKey::LushClayOre);
        features.extend((0..self.disks.len()).map(PlacedUndergroundFeatureKey::Disk));
        features.extend((0..self.springs.len()).map(PlacedUndergroundFeatureKey::Spring));
        features.push(PlacedUndergroundFeatureKey::MultifaceGrowth);
        features.extend(
            (0..self.environment_scan_features.len())
                .map(PlacedUndergroundFeatureKey::EnvironmentScan),
        );
        features.push(PlacedUndergroundFeatureKey::CaveVines);
        features.push(PlacedUndergroundFeatureKey::SporeBlossom);
        features.push(PlacedUndergroundFeatureKey::ClassicVines);
        features.extend((0..self.aquatic_features.len()).map(PlacedUndergroundFeatureKey::Aquatic));
        features
            .extend((0..self.huge_mushrooms.len()).map(PlacedUndergroundFeatureKey::HugeMushroom));
        features.extend(
            (0..self.vegetation_patches.len()).map(PlacedUndergroundFeatureKey::SimpleVegetation),
        );
        features.push(PlacedUndergroundFeatureKey::SurfaceVines);
        features
            .extend((0..self.block_columns.len()).map(PlacedUndergroundFeatureKey::BlockColumn));
        features.extend((0..self.trees.len()).map(PlacedUndergroundFeatureKey::Tree));
        features.push(PlacedUndergroundFeatureKey::FreezeTopLayer);
        features.sort_by_key(|key| {
            let feature = self.feature_by_key(*key);
            (feature.step_index(), feature.feature_index())
        });
        features
    }

    #[cfg(test)]
    fn ordered_features(&self) -> Vec<PlacedUndergroundFeature<'_>> {
        self.ordered_feature_keys
            .iter()
            .copied()
            .map(|key| self.feature_by_key(key))
            .collect()
    }

    fn feature_by_key(&self, key: PlacedUndergroundFeatureKey) -> PlacedUndergroundFeature<'_> {
        match key {
            PlacedUndergroundFeatureKey::Lake(index) => {
                PlacedUndergroundFeature::Lake(&self.lakes[index])
            }
            PlacedUndergroundFeatureKey::Geode(index) => {
                PlacedUndergroundFeature::Geode(&self.geodes[index])
            }
            PlacedUndergroundFeatureKey::Dripstone(index) => {
                PlacedUndergroundFeature::Dripstone(&self.dripstone_features[index])
            }
            PlacedUndergroundFeatureKey::Sculk(index) => {
                PlacedUndergroundFeature::Sculk(&self.sculk_features[index])
            }
            PlacedUndergroundFeatureKey::Structure(index) => {
                PlacedUndergroundFeature::Structure(&self.structure_features[index])
            }
            PlacedUndergroundFeatureKey::Surface(index) => {
                PlacedUndergroundFeature::Surface(&self.surface_features[index])
            }
            PlacedUndergroundFeatureKey::MonsterRoom(index) => {
                PlacedUndergroundFeature::MonsterRoom(&self.monster_rooms[index])
            }
            PlacedUndergroundFeatureKey::Ore(index) => {
                PlacedUndergroundFeature::Ore(&self.features[index])
            }
            PlacedUndergroundFeatureKey::UnderwaterMagma => {
                PlacedUndergroundFeature::UnderwaterMagma(&self.underwater_magma)
            }
            PlacedUndergroundFeatureKey::LushClayOre => {
                PlacedUndergroundFeature::Ore(&self.lush_clay_ore)
            }
            PlacedUndergroundFeatureKey::Disk(index) => {
                PlacedUndergroundFeature::Disk(&self.disks[index])
            }
            PlacedUndergroundFeatureKey::Spring(index) => {
                PlacedUndergroundFeature::Spring(&self.springs[index])
            }
            PlacedUndergroundFeatureKey::MultifaceGrowth => {
                PlacedUndergroundFeature::MultifaceGrowth(&self.glow_lichen)
            }
            PlacedUndergroundFeatureKey::EnvironmentScan(index) => {
                PlacedUndergroundFeature::EnvironmentScan(&self.environment_scan_features[index])
            }
            PlacedUndergroundFeatureKey::CaveVines => {
                PlacedUndergroundFeature::CaveVines(&self.cave_vines)
            }
            PlacedUndergroundFeatureKey::SporeBlossom => {
                PlacedUndergroundFeature::SporeBlossom(&self.spore_blossom)
            }
            PlacedUndergroundFeatureKey::ClassicVines => {
                PlacedUndergroundFeature::ClassicVines(&self.classic_vines)
            }
            PlacedUndergroundFeatureKey::Aquatic(index) => {
                PlacedUndergroundFeature::Aquatic(&self.aquatic_features[index])
            }
            PlacedUndergroundFeatureKey::HugeMushroom(index) => {
                PlacedUndergroundFeature::HugeMushroom(&self.huge_mushrooms[index])
            }
            PlacedUndergroundFeatureKey::SimpleVegetation(index) => {
                PlacedUndergroundFeature::SimpleVegetation(&self.vegetation_patches[index])
            }
            PlacedUndergroundFeatureKey::SurfaceVines => {
                PlacedUndergroundFeature::ClassicVines(&self.surface_vines)
            }
            PlacedUndergroundFeatureKey::BlockColumn(index) => {
                PlacedUndergroundFeature::BlockColumn(&self.block_columns[index])
            }
            PlacedUndergroundFeatureKey::Tree(index) => {
                PlacedUndergroundFeature::Tree(&self.trees[index])
            }
            PlacedUndergroundFeatureKey::FreezeTopLayer => {
                PlacedUndergroundFeature::FreezeTopLayer(&self.freeze_top_layer)
            }
        }
    }

    fn place_chunk(
        &self,
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
        chunk: &mut NoiseChunkBlocks,
    ) {
        let origin_x = chunk_x * 16;
        let origin_z = chunk_z * 16;
        let decoration_seed = FeatureRandom::decoration_seed(self.seed, origin_x, origin_z);
        let mut neighbor_sources = NeighborFeatureSources::new(self.seed, chunk_x, chunk_z);
        let profile_to_stderr = should_profile_to_stderr();
        let mut profile = (profile_to_stderr || log::log_enabled!(log::Level::Debug))
            .then(|| FeaturePlacementProfile::new(profile_to_stderr));

        let trace = FeatureTrace::from_env();

        for (ordinal, feature_key) in self.ordered_feature_keys.iter().copied().enumerate() {
            let feature = self.feature_by_key(feature_key);
            let feature_name = feature.name();
            let step_index = feature.step_index();
            let feature_index = feature.feature_index();
            let feature_label =
                format!("{feature_name}(step={step_index},index={feature_index},ord={ordinal})");
            let diagnostic_start = trace.start(
                chunk_x,
                chunk_z,
                ordinal,
                feature_name,
                step_index,
                feature_index,
            );
            let biome_filter = feature.biome_filter();
            if feature.can_spill_into_neighbor_chunk() {
                let candidate_indexes = neighbor_sources.candidates(feature);
                for source_index in candidate_indexes {
                    let prechecked_spillover = matches!(
                        feature,
                        PlacedUndergroundFeature::Dripstone(PlacedDripstoneFeature::Large(_))
                            | PlacedUndergroundFeature::MonsterRoom(_)
                    );
                    if prechecked_spillover
                        && !neighbor_sources.may_spill_without_loading(
                            settings,
                            source_index,
                            feature,
                        )
                    {
                        continue;
                    }
                    if !prechecked_spillover
                        && !neighbor_sources.can_match_biome(settings, source_index, biome_filter)
                    {
                        continue;
                    }
                    if let PlacedUndergroundFeature::MonsterRoom(feature) = feature {
                        let source_decoration_seed =
                            neighbor_sources.ensure_decoration_seed(source_index);
                        let source_origin_x = neighbor_sources.origin_x(source_index);
                        let source_origin_z = neighbor_sources.origin_z(source_index);
                        let mut random = FeatureRandom::for_feature(
                            source_decoration_seed,
                            feature.feature_index,
                            feature.step_index,
                        );
                        if feature.target_precheck_prevents_spillover(
                            settings,
                            source_origin_x,
                            source_origin_z,
                            origin_x,
                            origin_z,
                            chunk,
                            &mut random,
                        ) {
                            continue;
                        }
                    }

                    let load_start = Instant::now();
                    let can_place_spillover = if prechecked_spillover {
                        neighbor_sources.ensure_loaded(settings, source_index);
                        true
                    } else if feature.needs_source_neighbor_context() {
                        neighbor_sources.prepare_for_neighbor_context_feature(
                            settings,
                            source_index,
                            feature,
                        )
                    } else {
                        neighbor_sources.prepare_for_feature(settings, source_index, feature)
                    };
                    if let Some(profile) = profile.as_mut() {
                        profile.record(
                            &feature_label,
                            FeatureProfilePhase::NeighborLoad,
                            load_start.elapsed(),
                        );
                    }
                    if !can_place_spillover {
                        continue;
                    }

                    let source_origin_x = neighbor_sources.origin_x(source_index);
                    let source_origin_z = neighbor_sources.origin_z(source_index);
                    let source_chunk_x = source_origin_x.div_euclid(16);
                    let source_chunk_z = source_origin_z.div_euclid(16);
                    let source_decoration_seed = neighbor_sources.decoration_seed(source_index);
                    let mut random = FeatureRandom::for_feature(
                        source_decoration_seed,
                        feature.feature_index(),
                        feature.step_index(),
                    );
                    let spillover_start = Instant::now();
                    let trace_context = FeatureWriteTraceContext {
                        chunk_x,
                        chunk_z,
                        ordinal,
                        feature_name,
                        step_index,
                        feature_index,
                        phase: "spillover",
                        source_chunk_x,
                        source_chunk_z,
                        source_origin_x,
                        source_origin_z,
                        target_chunk_x: chunk_x,
                        target_chunk_z: chunk_z,
                        target_origin_x: origin_x,
                        target_origin_z: origin_z,
                        decoration_seed: source_decoration_seed,
                        attempt: None,
                        attempt_origin: None,
                    };
                    with_feature_write_trace_context(trace_context, || match feature {
                        PlacedUndergroundFeature::MonsterRoom(feature) => {
                            let mut source = neighbor_sources.take_source(source_index);
                            feature.place_with_spillover_lazy_neighbors(
                                settings,
                                source_origin_x,
                                source_origin_z,
                                origin_x,
                                origin_z,
                                source.chunk_mut(),
                                chunk,
                                &mut random,
                                &mut neighbor_sources,
                                &mut profile,
                                feature_name,
                            );
                            neighbor_sources.restore_source(source_index, source);
                        }
                        PlacedUndergroundFeature::Structure(feature) => {
                            let mut source = neighbor_sources.take_source(source_index);
                            let source_neighbors = neighbor_sources.context_chunks();
                            feature.place_with_spillover_neighbors(
                                settings,
                                source_origin_x,
                                source_origin_z,
                                origin_x,
                                origin_z,
                                source.chunk_mut(),
                                chunk,
                                &source_neighbors,
                                &mut random,
                            );
                            neighbor_sources.restore_source(source_index, source);
                        }
                        PlacedUndergroundFeature::HugeMushroom(feature) => {
                            let mut source = neighbor_sources.take_source(source_index);
                            let source_neighbors = neighbor_sources.context_chunks();
                            feature.place_with_spillover_neighbors(
                                settings,
                                source_origin_x,
                                source_origin_z,
                                origin_x,
                                origin_z,
                                source.chunk_mut(),
                                chunk,
                                &source_neighbors,
                                &mut random,
                            );
                            neighbor_sources.restore_source(source_index, source);
                        }
                        PlacedUndergroundFeature::Tree(feature) => {
                            let mut source = neighbor_sources.take_source(source_index);
                            let source_neighbors = neighbor_sources.context_chunks();
                            feature.place_with_spillover_neighbors(
                                settings,
                                source_origin_x,
                                source_origin_z,
                                origin_x,
                                origin_z,
                                source.chunk_mut(),
                                chunk,
                                &source_neighbors,
                                &mut random,
                            );
                            neighbor_sources.restore_source(source_index, source);
                        }
                        PlacedUndergroundFeature::Ore(feature) => {
                            let mut source = neighbor_sources.take_source(source_index);
                            let source_neighbors = neighbor_sources.all_context(settings);
                            feature.place_with_spillover_context(
                                settings,
                                source_origin_x,
                                source_origin_z,
                                origin_x,
                                origin_z,
                                source.chunk_mut(),
                                chunk,
                                &source_neighbors,
                                &mut random,
                            );
                            neighbor_sources.restore_source(source_index, source);
                        }
                        _ => {
                            let source = neighbor_sources.source_mut(source_index);
                            feature.place_spillover_from(
                                settings,
                                source_origin_x,
                                source_origin_z,
                                origin_x,
                                origin_z,
                                source.chunk_mut(),
                                chunk,
                                &mut random,
                            )
                        }
                    });
                    if let Some(profile) = profile.as_mut() {
                        profile.record(
                            &feature_label,
                            FeatureProfilePhase::Spillover,
                            spillover_start.elapsed(),
                        );
                    }
                }
            }
            if biome_filter.can_match_chunk(chunk) {
                let mut random = FeatureRandom::for_feature(
                    decoration_seed,
                    feature.feature_index(),
                    feature.step_index(),
                );
                let trace_context = FeatureWriteTraceContext {
                    chunk_x,
                    chunk_z,
                    ordinal,
                    feature_name,
                    step_index,
                    feature_index,
                    phase: "local",
                    source_chunk_x: chunk_x,
                    source_chunk_z: chunk_z,
                    source_origin_x: origin_x,
                    source_origin_z: origin_z,
                    target_chunk_x: chunk_x,
                    target_chunk_z: chunk_z,
                    target_origin_x: origin_x,
                    target_origin_z: origin_z,
                    decoration_seed,
                    attempt: None,
                    attempt_origin: None,
                };
                let local_elapsed =
                    with_feature_write_trace_context(trace_context, || match feature {
                        PlacedUndergroundFeature::MultifaceGrowth(feature) => {
                            let context_start = Instant::now();
                            let neighbor_chunks = neighbor_sources.all_context(settings);
                            if let Some(profile) = profile.as_mut() {
                                profile.record(
                                    &feature_label,
                                    FeatureProfilePhase::NeighborLoad,
                                    context_start.elapsed(),
                                );
                            }
                            let local_start = Instant::now();
                            feature.place_with_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &neighbor_chunks,
                                &mut random,
                            );
                            local_start.elapsed()
                        }
                        PlacedUndergroundFeature::ClassicVines(feature) => {
                            let context_start = Instant::now();
                            let neighbor_chunks = neighbor_sources.all_context(settings);
                            if let Some(profile) = profile.as_mut() {
                                profile.record(
                                    &feature_label,
                                    FeatureProfilePhase::NeighborLoad,
                                    context_start.elapsed(),
                                );
                            }
                            let local_start = Instant::now();
                            feature.place_with_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &neighbor_chunks,
                                &mut random,
                            );
                            local_start.elapsed()
                        }
                        PlacedUndergroundFeature::BlockColumn(feature) => {
                            let context_start = Instant::now();
                            let neighbor_chunks = neighbor_sources.all_context(settings);
                            if let Some(profile) = profile.as_mut() {
                                profile.record(
                                    &feature_label,
                                    FeatureProfilePhase::NeighborLoad,
                                    context_start.elapsed(),
                                );
                            }
                            let local_start = Instant::now();
                            feature.place_with_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &neighbor_chunks,
                                &mut random,
                            );
                            local_start.elapsed()
                        }
                        PlacedUndergroundFeature::SimpleVegetation(feature) => {
                            let context_start = Instant::now();
                            let neighbor_chunks = neighbor_sources.all_context(settings);
                            if let Some(profile) = profile.as_mut() {
                                profile.record(
                                    &feature_label,
                                    FeatureProfilePhase::NeighborLoad,
                                    context_start.elapsed(),
                                );
                            }
                            let local_start = Instant::now();
                            feature.place_with_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &neighbor_chunks,
                                &mut random,
                            );
                            local_start.elapsed()
                        }
                        PlacedUndergroundFeature::MonsterRoom(feature) => feature
                            .place_with_lazy_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &mut random,
                                &mut neighbor_sources,
                                &mut profile,
                                feature_name,
                            ),
                        PlacedUndergroundFeature::Structure(feature) => {
                            let context_start = Instant::now();
                            let neighbor_chunks = neighbor_sources.all_context(settings);
                            if let Some(profile) = profile.as_mut() {
                                profile.record(
                                    &feature_label,
                                    FeatureProfilePhase::NeighborLoad,
                                    context_start.elapsed(),
                                );
                            }
                            let local_start = Instant::now();
                            feature.place_with_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &neighbor_chunks,
                                &mut random,
                            );
                            local_start.elapsed()
                        }
                        PlacedUndergroundFeature::HugeMushroom(feature) => {
                            let context_start = Instant::now();
                            let neighbor_chunks = neighbor_sources.all_context(settings);
                            if let Some(profile) = profile.as_mut() {
                                profile.record(
                                    &feature_label,
                                    FeatureProfilePhase::NeighborLoad,
                                    context_start.elapsed(),
                                );
                            }
                            let local_start = Instant::now();
                            feature.place_with_neighbors(
                                settings,
                                origin_x,
                                origin_z,
                                chunk,
                                &neighbor_chunks,
                                &mut random,
                            );
                            local_start.elapsed()
                        }
                        _ => {
                            let local_start = Instant::now();
                            feature.place(settings, origin_x, origin_z, chunk, &mut random);
                            local_start.elapsed()
                        }
                    });
                if let Some(profile) = profile.as_mut() {
                    profile.record(&feature_label, FeatureProfilePhase::Local, local_elapsed);
                }
            }
            if let Some(start) = diagnostic_start {
                trace.done(
                    chunk_x,
                    chunk_z,
                    ordinal,
                    feature_name,
                    step_index,
                    feature_index,
                    start,
                );
            }
            if trace.should_stop(
                chunk_x,
                chunk_z,
                ordinal,
                feature_name,
                step_index,
                feature_index,
            ) {
                break;
            }
        }

        if let Some(profile) = profile {
            profile.log(chunk_x, chunk_z);
        }
    }
}

#[derive(Debug, Default)]
struct FeaturePlacementProfile {
    entries: HashMap<String, FeaturePlacementProfileEntry>,
    stderr: bool,
}

impl FeaturePlacementProfile {
    fn new(stderr: bool) -> Self {
        Self {
            entries: HashMap::new(),
            stderr,
        }
    }

    fn record(&mut self, feature: &str, phase: FeatureProfilePhase, duration: Duration) {
        let entry = self.entries.entry(feature.to_string()).or_default();
        match phase {
            FeatureProfilePhase::Local => {
                entry.local += duration;
                entry.local_calls += 1;
            }
            FeatureProfilePhase::Spillover => {
                entry.spillover += duration;
                entry.spillover_calls += 1;
            }
            FeatureProfilePhase::NeighborLoad => {
                entry.neighbor_load += duration;
                entry.neighbor_load_calls += 1;
            }
        }
    }

    fn log(&self, chunk_x: i32, chunk_z: i32) {
        if self.entries.is_empty() {
            return;
        }

        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|(_, left), (_, right)| {
            right
                .total()
                .cmp(&left.total())
                .then_with(|| right.spillover.cmp(&left.spillover))
                .then_with(|| right.neighbor_load.cmp(&left.neighbor_load))
        });

        let summary = entries
            .into_iter()
            .take(FEATURE_PROFILE_TOP_COUNT)
            .map(|(name, entry)| {
                format!(
                    "{}:total={:.2},local={:.2}/{} spill={:.2}/{} load={:.2}/{}",
                    name,
                    duration_ms(entry.total()),
                    duration_ms(entry.local),
                    entry.local_calls,
                    duration_ms(entry.spillover),
                    entry.spillover_calls,
                    duration_ms(entry.neighbor_load),
                    entry.neighbor_load_calls,
                )
            })
            .collect::<Vec<_>>()
            .join("; ");

        let message =
            format!("vanilla_noise feature profile: chunk=({chunk_x}, {chunk_z}), top=[{summary}]");
        if self.stderr {
            eprintln!("{message}");
        }
        log::debug!("{message}");
    }
}

#[derive(Debug, Default)]
struct FeaturePlacementProfileEntry {
    local: Duration,
    spillover: Duration,
    neighbor_load: Duration,
    local_calls: usize,
    spillover_calls: usize,
    neighbor_load_calls: usize,
}

impl FeaturePlacementProfileEntry {
    fn total(&self) -> Duration {
        self.local + self.spillover + self.neighbor_load
    }
}

#[derive(Debug, Clone, Copy)]
enum FeatureProfilePhase {
    Local,
    Spillover,
    NeighborLoad,
}

#[derive(Debug, Default)]
struct FeatureSourceCache {
    inner: Mutex<FeatureSourceCacheInner>,
    ready: Condvar,
}

impl Clone for FeatureSourceCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl FeatureSourceCache {
    fn insert_generated(&self, chunk_x: i32, chunk_z: i32, chunk: NoiseChunkBlocks) {
        let key = (chunk_x, chunk_z);
        let mut inner = self.inner.lock().expect("feature source cache poisoned");
        inner.stats.generated_inserts += 1;
        inner.insert(key, Arc::new(chunk));
    }

    fn get_or_insert_with(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        build: impl FnOnce() -> NoiseChunkBlocks,
    ) -> Arc<NoiseChunkBlocks> {
        let key = (chunk_x, chunk_z);
        {
            let mut inner = self.inner.lock().expect("feature source cache poisoned");
            loop {
                if let Some(chunk) = inner.chunks.get(&key).cloned() {
                    inner.stats.hits += 1;
                    inner.touch(key);
                    return chunk;
                }

                if inner.in_progress.insert(key) {
                    inner.stats.misses += 1;
                    break;
                }

                inner.stats.waits += 1;
                inner = self
                    .ready
                    .wait(inner)
                    .expect("feature source cache poisoned");
            }
        }

        let chunk = Arc::new(build());
        let mut inner = self.inner.lock().expect("feature source cache poisoned");
        inner.insert(key, chunk.clone());
        inner.in_progress.remove(&key);
        self.ready.notify_all();
        chunk
    }

    fn snapshot(&self) -> FeatureSourceCacheSnapshot {
        let inner = self.inner.lock().expect("feature source cache poisoned");
        FeatureSourceCacheSnapshot {
            len: inner.chunks.len(),
            in_progress: inner.in_progress.len(),
            hits: inner.stats.hits,
            misses: inner.stats.misses,
            waits: inner.stats.waits,
            evictions: inner.stats.evictions,
            generated_inserts: inner.stats.generated_inserts,
        }
    }
}

#[derive(Debug, Default)]
struct FeatureSourceCacheInner {
    chunks: HashMap<(i32, i32), Arc<NoiseChunkBlocks>>,
    in_progress: HashSet<(i32, i32)>,
    order: VecDeque<(i32, i32)>,
    stats: FeatureSourceCacheStats,
}

#[derive(Debug, Default)]
struct FeatureSourceCacheStats {
    hits: usize,
    misses: usize,
    waits: usize,
    evictions: usize,
    generated_inserts: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct FeatureSourceCacheSnapshot {
    len: usize,
    in_progress: usize,
    hits: usize,
    misses: usize,
    waits: usize,
    evictions: usize,
    generated_inserts: usize,
}

#[derive(Clone, Copy, Debug)]
enum PlacedUndergroundFeatureKey {
    Lake(usize),
    Geode(usize),
    Dripstone(usize),
    Sculk(usize),
    Structure(usize),
    Surface(usize),
    MonsterRoom(usize),
    Ore(usize),
    UnderwaterMagma,
    LushClayOre,
    Disk(usize),
    Spring(usize),
    MultifaceGrowth,
    CaveVines,
    ClassicVines,
    SporeBlossom,
    EnvironmentScan(usize),
    Aquatic(usize),
    HugeMushroom(usize),
    SimpleVegetation(usize),
    SurfaceVines,
    BlockColumn(usize),
    Tree(usize),
    FreezeTopLayer,
}

impl FeatureSourceCacheInner {
    fn touch(&mut self, key: (i32, i32)) {
        self.order.retain(|existing| *existing != key);
        self.order.push_back(key);
    }

    fn insert(&mut self, key: (i32, i32), chunk: Arc<NoiseChunkBlocks>) {
        self.chunks.insert(key, chunk);
        self.touch(key);
        while self.chunks.len() > FEATURE_SOURCE_CACHE_LIMIT {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if oldest != key {
                if self.chunks.remove(&oldest).is_some() {
                    self.stats.evictions += 1;
                }
            }
        }
    }
}

struct NeighborFeatureSources {
    seed: i64,
    target_origin_x: i32,
    target_origin_z: i32,
    all_loaded: bool,
    entries: Vec<Option<NeighborFeatureSource>>,
}

impl NeighborFeatureSources {
    fn new(seed: i64, chunk_x: i32, chunk_z: i32) -> Self {
        let mut entries = Vec::with_capacity(8);
        for dx in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                entries.push(Some(NeighborFeatureSource::placeholder(
                    chunk_x + dx,
                    chunk_z + dz,
                )));
            }
        }
        Self {
            seed,
            target_origin_x: chunk_x * 16,
            target_origin_z: chunk_z * 16,
            all_loaded: false,
            entries,
        }
    }

    fn all_context(&mut self, settings: &NoiseSettings) -> Vec<(i32, i32, &NoiseChunkBlocks)> {
        self.ensure_all(settings);
        self.entries
            .iter()
            .filter_map(Option::as_ref)
            .map(|source| (source.origin_x, source.origin_z, source.chunk_ref()))
            .collect()
    }

    fn ensure_all(&mut self, settings: &NoiseSettings) {
        if self.all_loaded {
            return;
        }
        for index in 0..self.entries.len() {
            self.ensure_loaded(settings, index);
        }
        self.all_loaded = self
            .entries
            .iter()
            .all(|source| source.as_ref().is_some_and(|source| source.chunk.is_some()));
    }

    fn candidates(&self, feature: PlacedUndergroundFeature<'_>) -> Vec<usize> {
        let max_spillover = feature.max_horizontal_spillover();
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, source)| {
                let source = source.as_ref()?;
                (feature_can_reach_chunk(
                    feature,
                    source.origin_x,
                    source.origin_z,
                    self.target_origin_x,
                    self.target_origin_z,
                    max_spillover,
                ))
                .then_some(index)
            })
            .collect()
    }

    fn ensure_loaded(&mut self, settings: &NoiseSettings, index: usize) {
        let Some(source) = self.entries[index].as_mut() else {
            return;
        };
        if source.chunk.is_some() {
            return;
        }
        self.all_loaded = false;
        source.decoration_seed =
            FeatureRandom::decoration_seed(self.seed, source.origin_x, source.origin_z);
        source.chunk = Some(settings.feature_source_chunk(source.chunk_x, source.chunk_z));
    }

    fn can_match_biome(
        &mut self,
        settings: &NoiseSettings,
        index: usize,
        filter: FeatureBiomeFilter,
    ) -> bool {
        if matches!(filter, FeatureBiomeFilter::All) {
            return true;
        }

        let Some(source) = self.entries[index].as_mut() else {
            return false;
        };
        if let Some(chunk) = source.chunk.as_ref() {
            return filter.can_match_chunk(chunk);
        }
        let biomes = source.biome_sample.get_or_insert_with(|| {
            sample_chunk_biomes(&settings.density, source.chunk_x, source.chunk_z)
        });
        filter.can_match_biomes(biomes.iter().copied())
    }

    fn prepare_for_feature(
        &mut self,
        settings: &NoiseSettings,
        index: usize,
        feature: PlacedUndergroundFeature<'_>,
    ) -> bool {
        let Some(source) = self.entries[index].as_mut() else {
            return false;
        };
        if source.decoration_seed == 0 {
            source.decoration_seed =
                FeatureRandom::decoration_seed(self.seed, source.origin_x, source.origin_z);
        }
        if !feature.may_spill_from_seed(
            settings,
            source.origin_x,
            source.origin_z,
            self.target_origin_x,
            self.target_origin_z,
            source.decoration_seed,
        ) {
            return false;
        }
        if source.chunk.is_none() {
            source.chunk = Some(settings.feature_source_chunk(source.chunk_x, source.chunk_z));
        }
        true
    }

    fn may_spill_without_loading(
        &mut self,
        settings: &NoiseSettings,
        index: usize,
        feature: PlacedUndergroundFeature<'_>,
    ) -> bool {
        let Some(source) = self.entries[index].as_mut() else {
            return false;
        };
        if source.decoration_seed == 0 {
            source.decoration_seed =
                FeatureRandom::decoration_seed(self.seed, source.origin_x, source.origin_z);
        }
        feature.may_spill_from_seed(
            settings,
            source.origin_x,
            source.origin_z,
            self.target_origin_x,
            self.target_origin_z,
            source.decoration_seed,
        )
    }

    fn ensure_decoration_seed(&mut self, index: usize) -> i64 {
        let Some(source) = self.entries[index].as_mut() else {
            return 0;
        };
        if source.decoration_seed == 0 {
            source.decoration_seed =
                FeatureRandom::decoration_seed(self.seed, source.origin_x, source.origin_z);
        }
        source.decoration_seed
    }

    fn prepare_for_neighbor_context_feature(
        &mut self,
        settings: &NoiseSettings,
        index: usize,
        feature: PlacedUndergroundFeature<'_>,
    ) -> bool {
        let Some(source) = self.entries[index].as_mut() else {
            return false;
        };
        if source.decoration_seed == 0 {
            source.decoration_seed =
                FeatureRandom::decoration_seed(self.seed, source.origin_x, source.origin_z);
        }
        if !feature.may_spill_from_seed(
            settings,
            source.origin_x,
            source.origin_z,
            self.target_origin_x,
            self.target_origin_z,
            source.decoration_seed,
        ) {
            return false;
        }
        self.ensure_all(settings);
        true
    }

    fn context_chunks(&self) -> Vec<(i32, i32, &NoiseChunkBlocks)> {
        self.entries
            .iter()
            .filter_map(Option::as_ref)
            .filter_map(|source| {
                source
                    .chunk
                    .as_ref()
                    .map(|chunk| (source.origin_x, source.origin_z, chunk.as_ref()))
            })
            .collect()
    }

    fn context_for_box(
        &mut self,
        settings: &NoiseSettings,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) -> Vec<(i32, i32, &NoiseChunkBlocks)> {
        let indexes: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, source)| {
                let source = source.as_ref()?;
                horizontal_box_overlaps_chunk(
                    min_x,
                    max_x,
                    min_z,
                    max_z,
                    source.origin_x,
                    source.origin_z,
                )
                .then_some(index)
            })
            .collect();
        for index in indexes {
            self.ensure_loaded(settings, index);
        }
        self.context_chunks()
    }

    fn take_source(&mut self, index: usize) -> NeighborFeatureSource {
        self.entries[index]
            .take()
            .expect("neighbor source entry exists")
    }

    fn restore_source(&mut self, index: usize, source: NeighborFeatureSource) {
        debug_assert!(self.entries[index].is_none());
        self.entries[index] = Some(source);
        self.all_loaded = self
            .entries
            .iter()
            .all(|source| source.as_ref().is_some_and(|source| source.chunk.is_some()));
    }

    fn source_mut(&mut self, index: usize) -> &mut NeighborFeatureSource {
        self.entries[index]
            .as_mut()
            .expect("neighbor source entry exists")
    }

    fn origin_x(&self, index: usize) -> i32 {
        self.entries[index]
            .as_ref()
            .expect("neighbor source entry exists")
            .origin_x
    }

    fn origin_z(&self, index: usize) -> i32 {
        self.entries[index]
            .as_ref()
            .expect("neighbor source entry exists")
            .origin_z
    }

    fn decoration_seed(&self, index: usize) -> i64 {
        self.entries[index]
            .as_ref()
            .expect("neighbor source entry exists")
            .decoration_seed
    }
}

fn sample_chunk_biomes(density: &TerrainDensity, chunk_x: i32, chunk_z: i32) -> Vec<&'static str> {
    let mut biomes = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        for local_y in 0..4 {
            for local_x in 0..4 {
                for local_z in 0..4 {
                    let quart_x = chunk_x * 4 + local_x;
                    let quart_y = section_y * 4 + local_y;
                    let quart_z = chunk_z * 4 + local_z;
                    let biome = density.biome_at_quart(quart_x, quart_y, quart_z);
                    if !biomes.contains(&biome) {
                        biomes.push(biome);
                    }
                }
            }
        }
    }
    biomes
}

fn origin_distance_to_chunk(delta: i32) -> i32 {
    if delta == 0 { 0 } else { delta.abs() - 15 }
}

fn feature_can_reach_chunk(
    feature: PlacedUndergroundFeature<'_>,
    source_origin_x: i32,
    source_origin_z: i32,
    target_origin_x: i32,
    target_origin_z: i32,
    max_spillover: i32,
) -> bool {
    match feature {
        PlacedUndergroundFeature::Lake(_) => origin_box_can_reach_chunk(
            source_origin_x - 8,
            source_origin_x + 23,
            source_origin_z - 8,
            source_origin_z + 23,
            target_origin_x,
            target_origin_z,
        ),
        PlacedUndergroundFeature::Disk(feature) => origin_box_can_reach_chunk(
            source_origin_x - feature.radius.max,
            source_origin_x + 15 + feature.radius.max,
            source_origin_z - feature.radius.max,
            source_origin_z + 15 + feature.radius.max,
            target_origin_x,
            target_origin_z,
        ),
        _ => {
            origin_distance_to_chunk(source_origin_x - target_origin_x) <= max_spillover
                && origin_distance_to_chunk(source_origin_z - target_origin_z) <= max_spillover
        }
    }
}

fn origin_box_can_reach_chunk(
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    target_origin_x: i32,
    target_origin_z: i32,
) -> bool {
    horizontal_box_overlaps_chunk(min_x, max_x, min_z, max_z, target_origin_x, target_origin_z)
}

struct NeighborFeatureSource {
    chunk_x: i32,
    chunk_z: i32,
    origin_x: i32,
    origin_z: i32,
    decoration_seed: i64,
    biome_sample: Option<Vec<&'static str>>,
    chunk: Option<Arc<NoiseChunkBlocks>>,
}

impl NeighborFeatureSource {
    fn placeholder(chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            chunk_x,
            chunk_z,
            origin_x: chunk_x * 16,
            origin_z: chunk_z * 16,
            decoration_seed: 0,
            biome_sample: None,
            chunk: None,
        }
    }

    fn chunk_ref(&self) -> &NoiseChunkBlocks {
        self.chunk
            .as_ref()
            .expect("neighbor feature source was loaded")
    }

    fn chunk_mut(&mut self) -> &mut NoiseChunkBlocks {
        Arc::make_mut(
            self.chunk
                .as_mut()
                .expect("neighbor feature source was loaded"),
        )
    }
}

#[derive(Clone, Copy)]
enum PlacedUndergroundFeature<'a> {
    Lake(&'a PlacedLakeFeature),
    Geode(&'a PlacedGeodeFeature),
    Dripstone(&'a PlacedDripstoneFeature),
    Sculk(&'a PlacedSculkFeature),
    Structure(&'a PlacedStructureFeature),
    Surface(&'a PlacedSurfaceFeature),
    MonsterRoom(&'a PlacedMonsterRoomFeature),
    Ore(&'a PlacedOreFeature),
    UnderwaterMagma(&'a PlacedUnderwaterMagmaFeature),
    Disk(&'a PlacedDiskFeature),
    Spring(&'a PlacedSpringFeature),
    MultifaceGrowth(&'a PlacedMultifaceGrowthFeature),
    CaveVines(&'a PlacedCaveVinesFeature),
    ClassicVines(&'a PlacedClassicVinesFeature),
    SporeBlossom(&'a PlacedSporeBlossomFeature),
    EnvironmentScan(&'a PlacedEnvironmentScanFeature),
    Aquatic(&'a PlacedAquaticFeature),
    HugeMushroom(&'a PlacedHugeMushroomFeature),
    SimpleVegetation(&'a PlacedSimpleVegetationFeature),
    BlockColumn(&'a PlacedBlockColumnFeature),
    Tree(&'a PlacedTreeFeature),
    FreezeTopLayer(&'a PlacedFreezeTopLayerFeature),
}

impl PlacedUndergroundFeature<'_> {
    fn name(self) -> &'static str {
        match self {
            Self::Lake(_) => "lake",
            Self::Geode(_) => "geode",
            Self::Dripstone(feature) => match feature {
                PlacedDripstoneFeature::Large(_) => "dripstone_large",
                PlacedDripstoneFeature::Cluster(_) => "dripstone_cluster",
                PlacedDripstoneFeature::Pointed(_) => "dripstone_pointed",
            },
            Self::Sculk(feature) => match feature {
                PlacedSculkFeature::Vein(_) => "sculk_vein",
                PlacedSculkFeature::Patch(_) => "sculk_patch",
            },
            Self::Structure(feature) => match feature {
                PlacedStructureFeature::DesertWell(_) => "structure_desert_well",
                PlacedStructureFeature::Fossil(_) => "structure_fossil",
            },
            Self::Surface(_) => "surface",
            Self::MonsterRoom(_) => "monster_room",
            Self::Ore(_) => "ore",
            Self::UnderwaterMagma(_) => "underwater_magma",
            Self::Disk(_) => "disk",
            Self::Spring(_) => "spring",
            Self::MultifaceGrowth(_) => "multiface_growth",
            Self::CaveVines(_) => "cave_vines",
            Self::ClassicVines(_) => "classic_vines",
            Self::SporeBlossom(_) => "spore_blossom",
            Self::EnvironmentScan(_) => "environment_scan",
            Self::Aquatic(_) => "aquatic",
            Self::HugeMushroom(_) => "huge_mushroom",
            Self::SimpleVegetation(_) => "simple_vegetation",
            Self::BlockColumn(_) => "block_column",
            Self::Tree(_) => "tree",
            Self::FreezeTopLayer(_) => "freeze_top_layer",
        }
    }

    fn step_index(self) -> i32 {
        match self {
            Self::Lake(feature) => feature.step_index,
            Self::Geode(feature) => feature.step_index,
            Self::Dripstone(feature) => feature.step_index(),
            Self::Sculk(feature) => feature.step_index(),
            Self::Structure(feature) => feature.step_index(),
            Self::Surface(feature) => feature.step_index,
            Self::MonsterRoom(feature) => feature.step_index,
            Self::Ore(feature) => feature.step_index,
            Self::UnderwaterMagma(feature) => feature.step_index,
            Self::Disk(feature) => feature.step_index,
            Self::Spring(feature) => feature.step_index,
            Self::MultifaceGrowth(feature) => feature.step_index,
            Self::CaveVines(feature) => feature.step_index,
            Self::ClassicVines(feature) => feature.step_index,
            Self::SporeBlossom(feature) => feature.step_index,
            Self::EnvironmentScan(feature) => feature.step_index,
            Self::Aquatic(feature) => feature.step_index,
            Self::HugeMushroom(feature) => feature.step_index,
            Self::SimpleVegetation(feature) => feature.step_index,
            Self::BlockColumn(feature) => feature.step_index,
            Self::Tree(feature) => feature.step_index,
            Self::FreezeTopLayer(feature) => feature.step_index,
        }
    }

    fn feature_index(self) -> i32 {
        match self {
            Self::Lake(feature) => feature.feature_index,
            Self::Geode(feature) => feature.feature_index,
            Self::Dripstone(feature) => feature.feature_index(),
            Self::Sculk(feature) => feature.feature_index(),
            Self::Structure(feature) => feature.feature_index(),
            Self::Surface(feature) => feature.feature_index,
            Self::MonsterRoom(feature) => feature.feature_index,
            Self::Ore(feature) => feature.feature_index,
            Self::UnderwaterMagma(feature) => feature.feature_index,
            Self::Disk(feature) => feature.feature_index,
            Self::Spring(feature) => feature.feature_index,
            Self::MultifaceGrowth(feature) => feature.feature_index,
            Self::CaveVines(feature) => feature.feature_index,
            Self::ClassicVines(feature) => feature.feature_index,
            Self::SporeBlossom(feature) => feature.feature_index,
            Self::EnvironmentScan(feature) => feature.feature_index,
            Self::Aquatic(feature) => feature.feature_index,
            Self::HugeMushroom(feature) => feature.feature_index,
            Self::SimpleVegetation(feature) => feature.feature_index,
            Self::BlockColumn(feature) => feature.feature_index,
            Self::Tree(feature) => feature.feature_index,
            Self::FreezeTopLayer(feature) => feature.feature_index,
        }
    }

    fn biome_filter(self) -> FeatureBiomeFilter {
        match self {
            Self::Lake(feature) => feature.biome_filter,
            Self::Geode(feature) => feature.biome_filter,
            Self::Dripstone(feature) => match feature {
                PlacedDripstoneFeature::Large(feature) => feature.biome_filter,
                PlacedDripstoneFeature::Cluster(feature) => feature.biome_filter,
                PlacedDripstoneFeature::Pointed(feature) => feature.biome_filter,
            },
            Self::Sculk(feature) => match feature {
                PlacedSculkFeature::Vein(feature) => feature.biome_filter,
                PlacedSculkFeature::Patch(feature) => feature.biome_filter,
            },
            Self::Structure(feature) => match feature {
                PlacedStructureFeature::DesertWell(feature) => feature.biome_filter,
                PlacedStructureFeature::Fossil(feature) => feature.biome_filter,
            },
            Self::Surface(feature) => feature.biome_filter,
            Self::MonsterRoom(feature) => feature.biome_filter,
            Self::Ore(feature) => feature.biome_filter,
            Self::UnderwaterMagma(feature) => feature.biome_filter,
            Self::Disk(feature) => feature.biome_filter,
            Self::Spring(feature) => feature.biome_filter,
            Self::MultifaceGrowth(feature) => feature.biome_filter,
            Self::CaveVines(feature) => feature.biome_filter,
            Self::ClassicVines(feature) => feature.biome_filter,
            Self::SporeBlossom(feature) => feature.biome_filter,
            Self::EnvironmentScan(feature) => feature.biome_filter,
            Self::Aquatic(feature) => feature.biome_filter,
            Self::HugeMushroom(feature) => feature.biome_filter,
            Self::SimpleVegetation(feature) => feature.biome_filter,
            Self::BlockColumn(feature) => feature.biome_filter,
            Self::Tree(feature) => feature.biome_filter,
            Self::FreezeTopLayer(_) => FeatureBiomeFilter::All,
        }
    }

    fn can_spill_into_neighbor_chunk(self) -> bool {
        !matches!(
            self,
            Self::Spring(_) | Self::CaveVines(_) | Self::SporeBlossom(_) | Self::FreezeTopLayer(_)
        )
    }

    fn max_horizontal_spillover(self) -> i32 {
        match self {
            Self::Lake(_) => 8,
            Self::Geode(_) => 16,
            Self::Dripstone(_) => 20,
            Self::Sculk(_) => 8,
            Self::Structure(_) => 16,
            Self::Surface(_) => 16,
            Self::MonsterRoom(_) => 4,
            Self::Ore(feature) => feature.ore.max_horizontal_spillover(),
            Self::UnderwaterMagma(feature) => feature.placement_radius_around_floor,
            Self::Disk(feature) => feature.radius.max,
            Self::MultifaceGrowth(_) => 20,
            Self::ClassicVines(_) => 1,
            Self::EnvironmentScan(_) => 8,
            Self::Aquatic(feature) => feature.max_horizontal_spillover(),
            Self::HugeMushroom(_) => 4,
            Self::SimpleVegetation(feature) => feature.max_horizontal_spillover(),
            Self::BlockColumn(feature) => feature.max_horizontal_spillover(),
            Self::Tree(feature) => feature.max_horizontal_spillover(),
            Self::Spring(_)
            | Self::CaveVines(_)
            | Self::SporeBlossom(_)
            | Self::FreezeTopLayer(_) => 0,
        }
    }

    fn can_precheck_spillover_without_source(self) -> bool {
        !matches!(self, Self::BlockColumn(_))
    }

    fn may_spill_from_seed(
        self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        decoration_seed: i64,
    ) -> bool {
        if !self.can_precheck_spillover_without_source() {
            return true;
        }

        let mut random =
            FeatureRandom::for_feature(decoration_seed, self.feature_index(), self.step_index());
        self.may_spill_from_random(
            settings,
            source_origin_x,
            source_origin_z,
            target_origin_x,
            target_origin_z,
            &mut random,
        )
    }

    fn may_spill_from_random(
        self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        random: &mut FeatureRandom,
    ) -> bool {
        match self {
            Self::Lake(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Geode(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Ore(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::UnderwaterMagma(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Disk(feature) => feature.may_spill_into(
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::MultifaceGrowth(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::EnvironmentScan(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Aquatic(feature) => feature.may_spill_into(
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::SimpleVegetation(feature) => feature.may_spill_into(
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::MonsterRoom(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Tree(feature) => feature.may_spill_into(
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Structure(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Surface(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Dripstone(feature) => feature.may_spill_into(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::HugeMushroom(feature) => feature.may_spill_into(
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                random,
            ),
            Self::Spring(_)
            | Self::CaveVines(_)
            | Self::ClassicVines(_)
            | Self::SporeBlossom(_)
            | Self::Sculk(_)
            | Self::BlockColumn(_)
            | Self::FreezeTopLayer(_) => true,
        }
    }

    fn needs_source_neighbor_context(self) -> bool {
        matches!(
            self,
            Self::Structure(_) | Self::HugeMushroom(_) | Self::Tree(_)
        )
    }

    fn place(
        self,
        settings: &NoiseSettings,
        origin_x: i32,
        origin_z: i32,
        chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::Lake(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Geode(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Dripstone(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Sculk(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Structure(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Surface(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::MonsterRoom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Ore(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::UnderwaterMagma(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Disk(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::Spring(feature) => feature.place(settings, origin_x, origin_z, chunk, random),
            Self::MultifaceGrowth(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::CaveVines(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::ClassicVines(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::SporeBlossom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::EnvironmentScan(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Aquatic(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::HugeMushroom(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::SimpleVegetation(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::BlockColumn(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::Tree(feature) => {
                feature.place(settings, origin_x, origin_z, chunk, random);
            }
            Self::FreezeTopLayer(feature) => feature.place(settings, origin_x, origin_z, chunk),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_spillover_from(
        self,
        settings: &NoiseSettings,
        source_origin_x: i32,
        source_origin_z: i32,
        target_origin_x: i32,
        target_origin_z: i32,
        source_chunk: &mut NoiseChunkBlocks,
        target_chunk: &mut NoiseChunkBlocks,
        random: &mut FeatureRandom,
    ) {
        match self {
            Self::Lake(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Geode(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Dripstone(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Sculk(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Structure(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Ore(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Spring(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::UnderwaterMagma(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Disk(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::MultifaceGrowth(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Surface(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::MonsterRoom(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::EnvironmentScan(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::Aquatic(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::HugeMushroom(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::SimpleVegetation(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::BlockColumn(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            Self::ClassicVines(feature) => feature.place_with_neighbors(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                &[(target_origin_x, target_origin_z, &*target_chunk)],
                random,
            ),
            Self::Tree(feature) => feature.place_with_spillover(
                settings,
                source_origin_x,
                source_origin_z,
                target_origin_x,
                target_origin_z,
                source_chunk,
                target_chunk,
                random,
            ),
            _ => self.place(
                settings,
                source_origin_x,
                source_origin_z,
                source_chunk,
                random,
            ),
        }
    }
}
