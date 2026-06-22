use super::*;

pub(crate) fn generate_overworld_chunk_nbt(seed: i64, chunk_x: i32, chunk_z: i32) -> Result<Tag> {
    let settings = cached_noise_settings(DEFAULT_NOISE_PRESET, seed);
    let stage = std::env::var("QEXED_WORLDGEN_V4_STAGE").unwrap_or_else(|_| "full".to_string());
    let chunk = match stage.as_str() {
        "base" => settings.generate_base_chunk(chunk_x, chunk_z).0,
        "carvers" => {
            let (mut chunk, preliminary_surfaces) = settings.generate_base_chunk(chunk_x, chunk_z);
            settings.carvers.carve_chunk(
                &settings,
                chunk_x,
                chunk_z,
                &preliminary_surfaces,
                &mut chunk,
            );
            chunk.recompute_first_available_heights(settings.min_y, settings.height);
            chunk
        }
        _ => settings.generate_chunk_profiled(chunk_x, chunk_z).0,
    };
    let mut root = noise_chunk_root(chunk_x, chunk_z, &chunk, settings.biome.as_str());
    append_block_entities_to_chunk_root(&mut root, &chunk);
    Ok(root)
}

fn cached_noise_settings(preset: &str, seed: i64) -> Arc<NoiseSettings> {
    static SETTINGS: OnceLock<std::sync::Mutex<HashMap<(String, i64), Arc<NoiseSettings>>>> =
        OnceLock::new();
    let key = (preset.to_string(), seed);
    let cache = SETTINGS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));

    if let Some(settings) = cache
        .lock()
        .expect("noise settings cache poisoned")
        .get(&key)
        .cloned()
    {
        return settings;
    }

    match load_noise_settings(preset, seed) {
        Ok(settings) => {
            let settings = Arc::new(settings);
            cache
                .lock()
                .expect("noise settings cache poisoned")
                .insert(key, settings.clone());
            settings
        }
        Err(_) => Arc::new(NoiseSettings::overworld(
            seed,
            vanilla_noise::OverworldNoiseKind::Default,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_v4_generation_uses_cached_settings_smoke() {
        let start = Instant::now();
        let first = generate_overworld_chunk_nbt(0, 0, 0).unwrap();
        let first_elapsed = start.elapsed();

        let start = Instant::now();
        let second = generate_overworld_chunk_nbt(0, 1, 0).unwrap();
        let second_elapsed = start.elapsed();

        assert!(matches!(first, Tag::Compound(_)));
        assert!(matches!(second, Tag::Compound(_)));
        eprintln!(
            "v4 cached settings smoke: first_ms={:.2}, second_ms={:.2}",
            duration_ms(first_elapsed),
            duration_ms(second_elapsed)
        );
    }

    #[test]
    fn generated_chunk_root_writes_post_processing_per_section() {
        let root = generate_overworld_chunk_nbt(0, 0, 0).unwrap();
        let Tag::Compound(fields) = root else {
            panic!("generated chunk root should be compound");
        };
        let Some(Tag::List(header, sections)) = fields.get("PostProcessing") else {
            panic!("PostProcessing should be a list");
        };

        assert_eq!(header.tag_id, tag_id::LIST);
        assert_eq!(header.length, section_count());
        assert_eq!(sections.len(), section_count() as usize);
        assert!(sections.iter().all(|section| {
            matches!(
                section,
                Tag::List(
                    ListHeader {
                        tag_id: tag_id::END,
                        length: 0,
                    },
                    _
                )
            )
        }));
    }

    #[test]
    #[ignore = "manual worldgen perf smoke"]
    fn worldgen_v4_perf_smoke_reports_stage_timings() {
        let settings = cached_noise_settings(DEFAULT_NOISE_PRESET, 0);
        let mut total = Duration::ZERO;
        let mut timings = NoiseChunkTimings {
            base: Duration::ZERO,
            carvers: Duration::ZERO,
            features: Duration::ZERO,
            heightmap: Duration::ZERO,
        };
        let mut chunks = 0u32;

        for chunk_x in -1..=1 {
            for chunk_z in -1..=1 {
                let start = Instant::now();
                let (chunk, chunk_timings) = settings.generate_chunk_profiled(chunk_x, chunk_z);
                total += start.elapsed();
                timings.base += chunk_timings.base;
                timings.carvers += chunk_timings.carvers;
                timings.features += chunk_timings.features;
                timings.heightmap += chunk_timings.heightmap;
                chunks += 1;
                assert_eq!(chunk.columns.len(), HEIGHTMAP_ENTRY_COUNT);
            }
        }

        let chunks = chunks as f64;
        let cache = settings.feature_source_cache.snapshot();
        eprintln!(
            "worldgen v4 perf smoke: chunks={}, avg_total_ms={:.2}, avg_base_ms={:.2}, avg_carvers_ms={:.2}, avg_features_ms={:.2}, avg_heightmap_ms={:.2}, cache_len={}, cache_hits={}, cache_misses={}, cache_waits={}, cache_evictions={}, generated_inserts={}",
            chunks as u32,
            duration_ms(total) / chunks,
            duration_ms(timings.base) / chunks,
            duration_ms(timings.carvers) / chunks,
            duration_ms(timings.features) / chunks,
            duration_ms(timings.heightmap) / chunks,
            cache.len,
            cache.hits,
            cache.misses,
            cache.waits,
            cache.evictions,
            cache.generated_inserts,
        );
    }
}
