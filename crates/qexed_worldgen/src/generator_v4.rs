#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use qexed_nbt::{ListHeader, Tag, tag_id};
use rayon::prelude::*;
use serde::Deserialize;

use crate::{
    WorldgenCache,
    constants::{DATA_VERSION, SECTION_HEIGHT as CRATE_SECTION_HEIGHT, WORLD_HEIGHT, WORLD_MIN_Y},
    vanilla_noise,
};

const WORLD_MAX_Y: i32 = WORLD_MIN_Y + WORLD_HEIGHT - 1;
const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / CRATE_SECTION_HEIGHT;
const WORLD_SECTION_COUNT: usize = (WORLD_HEIGHT / CRATE_SECTION_HEIGHT) as usize;
const SECTION_HEIGHT: i32 = CRATE_SECTION_HEIGHT;
const CHUNK_DAMPENING_LEN: usize = (WORLD_SECTION_COUNT + 2) * 16 * 16;

#[derive(Debug, Clone)]
struct OptionalNbt(Option<Tag>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VarInt(i32);

#[derive(Debug, Clone)]
struct BlockEntities {
    xz: u8,
    y: u16,
    entity_type: VarInt,
    nbt: OptionalNbt,
}

mod chunk_nbt;
mod pipeline;

include!("generator_v4/constants.rs");
include!("generator_v4/core_pure.rs");
include!("generator_v4/terrain.rs");
include!("generator_v4/dimensions.rs");
include!("generator_v4/carvers.rs");
include!("generator_v4/sampling.rs");
include!("generator_v4/features.rs");
include!("generator_v4/features_lakes.rs");
include!("generator_v4/features_geodes.rs");
include!("generator_v4/features_cave.rs");
include!("generator_v4/features_aquatic.rs");
include!("generator_v4/features_lush.rs");
include!("generator_v4/features_surface.rs");
include!("generator_v4/features_mushrooms.rs");
include!("generator_v4/features_dripstone.rs");
include!("generator_v4/features_sculk.rs");
include!("generator_v4/features_structures.rs");
include!("generator_v4/features_vegetation.rs");
include!("generator_v4/features_columns.rs");
include!("generator_v4/features_trees.rs");
include!("generator_v4/features_common.rs");
include!("generator_v4/features_ores.rs");
include!("generator_v4/support.rs");
include!("generator_v4/nbt.rs");
include!("generator_v4/tests.rs");

pub(crate) use self::pipeline::generate_overworld_chunk_nbt;

fn section_count() -> i32 {
    WORLD_SECTION_COUNT as i32
}

#[cfg(test)]
mod diagnostics {
    use super::*;

    #[test]
    #[ignore = "manual feature placement diagnostic"]
    fn dump_seed0_ore_gravel_origins_chunk00() {
        let settings = NoiseSettings::overworld(0, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.features[1];
        println!("== rust ore_gravel placement origins ==");
        for source_chunk_x in -1..=1 {
            for source_chunk_z in -1..=1 {
                let origin_x = source_chunk_x * 16;
                let origin_z = source_chunk_z * 16;
                let decoration_seed =
                    FeatureRandom::decoration_seed(settings.ore_features.seed, origin_x, origin_z);
                let mut random = FeatureRandom::for_feature(
                    decoration_seed,
                    feature.feature_index,
                    feature.step_index,
                );
                let count = feature.count.sample(&mut random);
                for attempt in 0..count {
                    let x = origin_x + random.next_int(16);
                    let z = origin_z + random.next_int(16);
                    let y = feature.height.sample(&settings, &mut random);
                    let mut ore_random = random.clone();
                    let reaches = feature.ore.may_spill_into(0, 0, &mut ore_random, x, y, z);
                    if reaches || source_chunk_x == 0 && source_chunk_z == 0 {
                        println!(
                            "source=({source_chunk_x},{source_chunk_z}) attempt={attempt} origin=({x},{y},{z}) reaches_chunk00={reaches}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "manual ore_tuff spillover precheck diagnostic"]
    fn dump_seed0_ore_tuff_source_1_minus1_chunk00_precheck() {
        let settings = NoiseSettings::overworld(0, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.features[8];
        let source_origin_x = 16;
        let source_origin_z = -16;
        let target_origin_x = 0;
        let target_origin_z = 0;
        let decoration_seed = FeatureRandom::decoration_seed(
            settings.ore_features.seed,
            source_origin_x,
            source_origin_z,
        );
        let underground = PlacedUndergroundFeature::Ore(feature);
        let candidate = feature_can_reach_chunk(
            underground,
            source_origin_x,
            source_origin_z,
            target_origin_x,
            target_origin_z,
            underground.max_horizontal_spillover(),
        );
        let may_spill = underground.may_spill_from_seed(
            &settings,
            source_origin_x,
            source_origin_z,
            target_origin_x,
            target_origin_z,
            decoration_seed,
        );
        println!(
            "ore_tuff_precheck seed=0 source_chunk=(1,-1) source_origin=({source_origin_x},{source_origin_z}) target_chunk=(0,0) target_origin=({target_origin_x},{target_origin_z}) step={} index={} decoration_seed={decoration_seed} candidate={candidate} may_spill_from_seed={may_spill} prepare_for_feature_equivalent={}",
            feature.step_index,
            feature.feature_index,
            candidate && may_spill
        );

        let mut random =
            FeatureRandom::for_feature(decoration_seed, feature.feature_index, feature.step_index);
        let count = feature.count.sample(&mut random);
        println!("ore_tuff_attempt_count={count}");
        for attempt in 0..count {
            let x = source_origin_x + random.next_int(16);
            let z = source_origin_z + random.next_int(16);
            let y = feature.height.sample(&settings, &mut random);
            let reaches = feature.ore.may_spill_into(
                target_origin_x,
                target_origin_z,
                &mut random,
                x,
                y,
                z,
            );
            println!(
                "ore_tuff_attempt attempt={attempt} origin=({x},{y},{z}) may_spill_into_target={reaches}"
            );
        }

        let seed_inputs = [
            ("origin", source_origin_x, source_origin_z),
            ("chunk", 1, -1),
            ("chunk16plus8", 24, -8),
        ];
        for (seed_label, seed_x, seed_z) in seed_inputs {
            let decoration_seed =
                FeatureRandom::decoration_seed(settings.ore_features.seed, seed_x, seed_z);
            for step_index in [5, 6, 7] {
                for feature_index in 6..=10 {
                    let mut random =
                        FeatureRandom::for_feature(decoration_seed, feature_index, step_index);
                    let count = feature.count.sample(&mut random);
                    let mut origins = Vec::new();
                    for attempt in 0..count {
                        let x = source_origin_x + random.next_int(16);
                        let z = source_origin_z + random.next_int(16);
                        let y = feature.height.sample(&settings, &mut random);
                        origins.push((attempt, x, y, z));
                    }
                    if origins
                        .iter()
                        .any(|(_, x, y, z)| (*x, *y, *z) == (19, -49, -5))
                    {
                        println!(
                            "ore_tuff_java_origin_match seed_input={seed_label} seed_coords=({seed_x},{seed_z}) step={step_index} index={feature_index} decoration_seed={decoration_seed} origins={origins:?}"
                        );
                    }
                }
            }
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))
}
