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
    let mut root = noise_chunk_root(&chunk, settings.biome.as_str());
    append_block_entities_to_chunk_root(&mut root, &chunk);
    normalize_chunk_root(root, chunk_x, chunk_z)
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

fn normalize_chunk_root(root: Tag, chunk_x: i32, chunk_z: i32) -> Result<Tag> {
    let Tag::Compound(fields) = root else {
        anyhow::bail!("generated chunk root is not compound");
    };
    let mut fields = (*fields).clone();
    fields.insert("DataVersion".to_string(), Tag::Int(DATA_VERSION));
    fields.insert("xPos".to_string(), Tag::Int(chunk_x));
    fields.insert("yPos".to_string(), Tag::Int(WORLD_MIN_SECTION_Y));
    fields.insert("zPos".to_string(), Tag::Int(chunk_z));
    fields.insert("LastUpdate".to_string(), Tag::Long(0));
    fields.insert("InhabitedTime".to_string(), Tag::Long(0));
    fields.insert(
        "Status".to_string(),
        Tag::String(Arc::from("minecraft:full")),
    );
    fields
        .entry("block_entities".to_string())
        .or_insert_with(empty_compound_list);
    fields.insert("block_ticks".to_string(), empty_compound_list());
    fields.insert("fluid_ticks".to_string(), empty_compound_list());
    fields.insert("PostProcessing".to_string(), empty_nested_list());
    fields.insert("structures".to_string(), structures_tag());
    fields.insert("isLightOn".to_string(), Tag::Byte(1));
    Ok(Tag::Compound(Arc::new(fields)))
}

fn empty_compound_list() -> Tag {
    Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: 0,
        },
        Arc::new([]),
    )
}

fn empty_nested_list() -> Tag {
    Tag::List(
        ListHeader {
            tag_id: tag_id::LIST,
            length: 0,
        },
        Arc::new([]),
    )
}

fn structures_tag() -> Tag {
    Tag::Compound(Arc::new(HashMap::from([
        (
            "starts".to_string(),
            Tag::Compound(Arc::new(HashMap::new())),
        ),
        (
            "References".to_string(),
            Tag::Compound(Arc::new(HashMap::new())),
        ),
    ])))
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
        eprintln!(
            "worldgen v4 perf smoke: chunks={}, avg_total_ms={:.2}, avg_base_ms={:.2}, avg_carvers_ms={:.2}, avg_features_ms={:.2}, avg_heightmap_ms={:.2}",
            chunks as u32,
            duration_ms(total) / chunks,
            duration_ms(timings.base) / chunks,
            duration_ms(timings.carvers) / chunks,
            duration_ms(timings.features) / chunks,
            duration_ms(timings.heightmap) / chunks,
        );
    }
}
