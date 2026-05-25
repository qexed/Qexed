#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::*;

    fn grass_surface_test_chunk(settings: &NoiseSettings, surface_y: i32) -> NoiseChunkBlocks {
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= surface_y {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: surface_y + 1 - settings.min_y,
            })
            .collect();

        NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        }
    }

    fn chunk_contains_block(chunk: &NoiseChunkBlocks, block: &str) -> bool {
        chunk
            .columns
            .iter()
            .any(|column| column.blocks.iter().any(|layer| layer.is(block)))
    }

    fn generate_noise_chunk_without_neighbor_tree_spillover(
        settings: &NoiseSettings,
        chunk_x: i32,
        chunk_z: i32,
    ) -> NoiseChunkBlocks {
        let origin_x = chunk_x * 16;
        let origin_z = chunk_z * 16;
        let decoration_seed = FeatureRandom::decoration_seed(settings.ore_features.seed, origin_x, origin_z);
        let (mut chunk, preliminary_surfaces) = settings.generate_base_chunk(chunk_x, chunk_z);
        settings.carvers.carve_chunk(
            settings,
            chunk_x,
            chunk_z,
            &preliminary_surfaces,
            &mut chunk,
        );

        for feature in settings.ore_features.ordered_features() {
            let mut random = FeatureRandom::for_feature(
                decoration_seed,
                feature.feature_index(),
                feature.step_index(),
            );
            feature.place(settings, origin_x, origin_z, &mut chunk, &mut random);
        }
        chunk
    }

    fn boundary_tree_block_count(chunk: &NoiseChunkBlocks) -> usize {
        let mut count = 0;
        for z in 0..16 {
            for x in 0..16 {
                if x != 0 && x != 15 && z != 0 && z != 15 {
                    continue;
                }
                count += chunk
                    .column(x, z)
                    .blocks
                    .iter()
                    .filter(|layer| is_log_layer(layer) || is_leaf_layer(layer))
                    .count();
            }
        }
        count
    }

    fn tree_feature(settings: &NoiseSettings, feature_index: i32) -> &PlacedTreeFeature {
        settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == feature_index)
            .unwrap()
    }

    fn assert_tree_feature(
        feature: &PlacedTreeFeature,
        count_entries: &[(i32, i32)],
        surface_water_depth: i32,
        biome_filter: &'static [&'static str],
    ) {
        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.count.entries, count_entries);
        assert_eq!(feature.surface_water_depth, surface_water_depth);
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == biome_filter
        ));
    }

    #[test]
    fn java_random_matches_legacy_lcg_outputs() {
        let mut random = JavaRandom::new(12345);

        assert_eq!(random.next_int(15), 1);
        assert_eq!(random.next_int(1000), 80);
        assert_eq!(random.next_float().to_bits(), 0.932_993_5_f32.to_bits());
        assert_eq!(random.next_long(), -1_528_963_862_231_680_626);

        random.set_large_feature_seed(12345, 3, -7);
        assert_eq!(random.next_int(16), 7);
        assert_eq!(random.next_float().to_bits(), 0.294_135_75_f32.to_bits());
    }

    #[test]
    fn vanilla_flat_uses_classic_flat_layers_from_min_y() {
        let generator = VanillaFlatGenerator::from_preset(DEFAULT_FLAT_PRESET);
        let bedrock = chunk_nbt::default_block_state_id("minecraft:bedrock");
        let dirt = chunk_nbt::default_block_state_id("minecraft:dirt");
        let grass = chunk_nbt::default_block_state_id("minecraft:grass_block");

        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y,
                    z: 0
                }
            ),
            Some(bedrock)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 2,
                    z: 0
                }
            ),
            Some(dirt)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 3,
                    z: 0
                }
            ),
            Some(grass)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 4,
                    z: 0
                }
            ),
            None
        );
    }

    #[test]
    fn vanilla_flat_generated_chunk_serializes() {
        let generator = VanillaFlatGenerator::from_preset(DEFAULT_FLAT_PRESET);
        let generated = generator
            .generate("minecraft:overworld", 0, 0, WorldLightAlgorithm::Fast)
            .unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        generated.packet.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
        assert_eq!(generated.light_dampening.len(), CHUNK_DAMPENING_LEN);
        assert_eq!(generated.packet.data.heightmaps.len(), 3);
    }

    #[test]
    fn flat_heightmap_uses_vanilla_first_available_offset() {
        let layers = expand_layers(FlatSettings::classic().layers);
        let packed = match heightmaps_tag(&layers) {
            Tag::Compound(fields) => fields
                .get("WORLD_SURFACE")
                .and_then(|tag| match tag {
                    Tag::LongArray(values) => Some(values.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!("heightmaps must be compound"),
        };

        assert_eq!(packed.len(), 37);
        assert_eq!(packed[0] & 0x1ff, 4);
    }

    #[test]
    fn vanilla_noise_generated_chunk_serializes() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 0,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);
        let generated = generator
            .generate("minecraft:overworld", 0, 0, WorldLightAlgorithm::Fast)
            .unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        generated.packet.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
        assert_eq!(generated.light_dampening.len(), CHUNK_DAMPENING_LEN);
        assert_eq!(generated.packet.data.heightmaps.len(), 3);
    }

    #[test]
    fn vanilla_noise_block_state_at_uses_seeded_terrain() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 12345,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);

        assert!(
            generator
                .block_state_at(
                    "minecraft:overworld",
                    &qexed_packet::net_types::Position { x: 0, y: -64, z: 0 }
                )
                .is_some()
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position { x: 0, y: 320, z: 0 }
            ),
            None
        );
    }

    #[test]
    fn vanilla_noise_applies_deepslate_surface_rule() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 12345,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);
        let deepslate = chunk_nbt::default_block_state_id("minecraft:deepslate");

        let has_deepslate = (-16..=16).any(|x| {
            (-16..=16).any(|z| {
                (-32..=0).any(|y| {
                    generator.block_state_at(
                        "minecraft:overworld",
                        &qexed_packet::net_types::Position { x, y, z },
                    ) == Some(deepslate)
                })
            })
        });

        assert!(has_deepslate);
    }

    #[test]
    fn vanilla_noise_low_dry_surface_uses_grass_not_sea_level_dirt() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let surface_height = 58;
        let layer = settings.layer_at_with_density(
            0,
            surface_height,
            0,
            1.0,
            surface_height,
            surface_height,
            0,
            None,
        );

        assert!(layer.is("minecraft:grass_block"));
    }

    #[test]
    fn vanilla_noise_uses_lava_below_global_fluid_cutoff() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);

        assert_eq!(
            settings.aquifer.substance_at(0, -55, 0, -1.0, 64),
            vanilla_noise::AquiferSubstance::Fluid(vanilla_noise::AquiferFluid::Lava)
        );
    }

    #[test]
    fn vanilla_noise_maps_ore_vein_blocks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let has_ore_vein_block = (-32..=32).any(|x| {
            (-32..=32).any(|z| (-60..=50).any(|y| settings.ore_vein_at(x, y, z).is_some()))
        });

        assert!(has_ore_vein_block);
    }

    #[test]
    fn vanilla_noise_carvers_modify_seeded_chunks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let (base, _) = settings.generate_base_chunk(0, 0);
        let carved = settings.generate_chunk(0, 0);
        let carved_air = (0..16).any(|z| {
            (0..16).any(|x| {
                let base_column = base.column(x, z);
                let carved_column = carved.column(x, z);
                base_column
                    .blocks
                    .iter()
                    .zip(&carved_column.blocks)
                    .any(|(before, after)| {
                        !before.is_air && before.block.as_ref() != "minecraft:lava" && after.is_air
                    })
            })
        });

        assert!(carved_air);
    }

    #[test]
    fn vanilla_noise_places_common_overworld_ores() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let deepslate = BlockLayer::new("minecraft:deepslate");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y < 0 {
                            deepslate.clone()
                        } else {
                            stone.clone()
                        }
                    })
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut found = HashSet::new();

        settings
            .ore_features
            .place_chunk(&settings, 0, 0, &mut chunk);
        for column in &chunk.columns {
            for layer in &column.blocks {
                match layer.block.as_ref() {
                    "minecraft:dirt"
                    | "minecraft:gravel"
                    | "minecraft:granite"
                    | "minecraft:diorite"
                    | "minecraft:andesite"
                    | "minecraft:tuff"
                    | "minecraft:coal_ore"
                    | "minecraft:deepslate_coal_ore"
                    | "minecraft:iron_ore"
                    | "minecraft:deepslate_iron_ore"
                    | "minecraft:copper_ore"
                    | "minecraft:deepslate_copper_ore"
                    | "minecraft:redstone_ore"
                    | "minecraft:deepslate_redstone_ore" => {
                        found.insert(layer.block.clone());
                    }
                    _ => {}
                }
            }
        }

        assert!(found.contains("minecraft:dirt"));
        assert!(found.contains("minecraft:gravel"));
        assert!(found.contains("minecraft:coal_ore"));
        assert!(
            found.contains("minecraft:iron_ore") || found.contains("minecraft:deepslate_iron_ore")
        );
        assert!(
            found.contains("minecraft:copper_ore")
                || found.contains("minecraft:deepslate_copper_ore")
        );
        assert!(
            found.contains("minecraft:redstone_ore")
                || found.contains("minecraft:deepslate_redstone_ore")
        );
    }

    #[test]
    fn vanilla_noise_configures_biome_filtered_overworld_ores() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.features;

        assert!(features.iter().any(|feature| {
            feature.feature_index == 24
                && feature.ore.size == 10
                && matches!(feature.biome_filter, FeatureBiomeFilter::Exclude(biomes) if biomes.contains(&"minecraft:dripstone_caves"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 24
                && feature.ore.size == 20
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:dripstone_caves"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 26
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block.as_ref() == "minecraft:gold_ore")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 29
                && feature.step_index == 6
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block.as_ref() == "minecraft:emerald_ore")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:meadow"))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 0
                && feature.step_index == 7
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| target.block.block.as_ref() == "minecraft:infested_stone")
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:meadow"))
        }));
    }

    #[test]
    fn vanilla_noise_configures_underwater_magma_and_soft_disks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let magma = &settings.ore_features.underwater_magma;
        let disks = &settings.ore_features.disks;

        assert_eq!(magma.step_index, 6);
        assert_eq!(magma.feature_index, 25);
        assert!(matches!(
            magma.count,
            OrePlacementCount::Uniform { min: 44, max: 52 }
        ));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 26
                && feature.target_blocks == DISK_DIRT_GRASS_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Exclude(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 27
                && feature.target_blocks == DISK_DIRT_GRASS_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 27
                && feature.target_blocks == DISK_DIRT_CLAY_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Exclude(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 29
                && feature.target_blocks == DISK_DIRT_GRASS_TARGETS
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes.contains(&"minecraft:badlands"))
        }));
    }

    #[test]
    fn vanilla_noise_configures_overworld_springs() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let springs = &settings.ore_features.springs;

        assert!(springs.iter().any(|feature| {
            feature.step_index == 8
                && feature.feature_index == 0
                && matches!(feature.count, OrePlacementCount::Constant(25))
                && feature.config.state.block.as_ref() == "minecraft:water"
        }));
        assert!(springs.iter().any(|feature| {
            feature.step_index == 8
                && feature.feature_index == 1
                && matches!(feature.count, OrePlacementCount::Constant(20))
                && matches!(
                    feature.height,
                    OreHeight::VeryBiasedToBottom {
                        min: HeightAnchor::AboveBottom(0),
                        max: HeightAnchor::BelowTop(8),
                        inner: 8
                    }
                )
                && feature.config.state.block.as_ref() == "minecraft:lava"
        }));
    }

    #[test]
    fn vanilla_noise_configures_lava_lakes() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let lakes = &settings.ore_features.lakes;

        assert!(lakes.iter().any(|feature| {
            feature.step_index == 1
                && feature.feature_index == 0
                && matches!(
                    feature.placement,
                    LakePlacement::Underground {
                        rarity: 9,
                        max_scan_steps: 32,
                        ..
                    }
                )
        }));
        assert!(lakes.iter().any(|feature| {
            feature.step_index == 1
                && feature.feature_index == 1
                && matches!(feature.placement, LakePlacement::Surface { rarity: 200 })
        }));
    }

    #[test]
    fn vanilla_noise_configures_amethyst_geodes() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let geodes = &settings.ore_features.geodes;

        assert!(geodes.iter().any(|feature| {
            feature.step_index == 2
                && feature.feature_index == 0
                && feature.rarity == 24
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(30))
                )
        }));
    }

    #[test]
    fn vanilla_noise_configures_monster_rooms() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let rooms = &settings.ore_features.monster_rooms;

        assert!(rooms.iter().any(|feature| {
            feature.step_index == 3
                && feature.feature_index == 0
                && matches!(feature.count, OrePlacementCount::Constant(10))
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0))
                )
        }));
        assert!(rooms.iter().any(|feature| {
            feature.step_index == 3
                && feature.feature_index == 1
                && matches!(feature.count, OrePlacementCount::Constant(4))
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::AboveBottom(6), HeightAnchor::Absolute(-1))
                )
        }));
    }

    #[test]
    fn vanilla_noise_configures_glow_lichen() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.glow_lichen;

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 0);
        assert_eq!(feature.max_below_ocean_floor, -13);
        assert!(matches!(
            feature.count,
            OrePlacementCount::Uniform { min: 104, max: 157 }
        ));
        assert!(matches!(
            feature.height,
            OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256))
        ));
        assert_eq!(feature.config.search_range, 20);
        assert!(feature.config.can_place_on_ceiling);
        assert!(!feature.config.can_place_on_floor);
        assert!(feature.config.can_place_on_wall);
        assert_eq!(feature.config.chance_of_spreading, 0.5);
        assert_eq!(
            feature.config.can_be_placed_on,
            GLOW_LICHEN_CAN_BE_PLACED_ON
        );
    }

    #[test]
    fn vanilla_noise_configures_patch_tall_grass_2() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 1)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 1);
        let noise_threshold = feature.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 0);
        assert_eq!(noise_threshold.above_noise, 7);
        assert_eq!(feature.rarity, 32);
        assert_eq!(feature.inner_count, 96);
        assert_eq!(feature.xz_offset.min, -7);
        assert_eq!(feature.xz_offset.max, 7);
        assert_eq!(feature.xz_offset.plateau, 0);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert_eq!(feature.y_offset.plateau, 0);
        assert!(feature.block.lower.is("minecraft:tall_grass"));
        assert!(
            feature
                .block
                .lower
                .properties
                .iter()
                .any(|(name, value)| { name == "half" && value == "lower" })
        );
        assert!(feature.block.upper.as_ref().is_some_and(|upper| {
            upper.is("minecraft:tall_grass")
                && upper
                    .properties
                    .iter()
                    .any(|(name, value)| name == "half" && value == "upper")
        }));
    }

    #[test]
    fn vanilla_noise_configures_patch_bush() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 2)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 2);
        assert!(feature.noise_threshold.is_none());
        assert_eq!(feature.rarity, 4);
        assert_eq!(feature.inner_count, 24);
        assert_eq!(feature.xz_offset.min, -5);
        assert_eq!(feature.xz_offset.max, 5);
        assert_eq!(feature.xz_offset.plateau, 0);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert_eq!(feature.y_offset.plateau, 0);
        assert!(feature.block.lower.is("minecraft:bush"));
        assert!(feature.block.upper.is_none());
    }

    #[test]
    fn vanilla_noise_configures_patch_sunflower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 3)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 3);
        assert!(feature.noise_threshold.is_none());
        assert_eq!(feature.rarity, 3);
        assert_eq!(feature.inner_count, 96);
        assert_eq!(feature.xz_offset.min, -7);
        assert_eq!(feature.xz_offset.max, 7);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert!(feature.block.lower.is("minecraft:sunflower"));
        assert!(feature.block.upper.as_ref().is_some_and(|upper| {
            upper.is("minecraft:sunflower")
                && upper
                    .properties
                    .iter()
                    .any(|(name, value)| name == "half" && value == "upper")
        }));
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes == SUNFLOWER_PATCH_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_flower_plains() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 4)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 4);
        let noise_threshold = feature.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 15);
        assert_eq!(noise_threshold.above_noise, 4);
        assert_eq!(feature.rarity, 32);
        assert_eq!(feature.inner_count, 64);
        assert_eq!(feature.xz_offset.min, -6);
        assert_eq!(feature.xz_offset.max, 6);
        assert_eq!(feature.y_offset.min, -2);
        assert_eq!(feature.y_offset.max, 2);
        assert!(feature.block.lower.is("minecraft:dandelion"));
        assert!(matches!(
            &feature.block.provider,
            SimpleVegetationProvider::PlainsFlower {
                high_chance: 0.333_333_34,
                threshold: -0.8,
                scale: 0.005,
                ..
            }
        ));
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:sunflower_plains")
        ));
    }

    #[test]
    fn vanilla_noise_configures_patch_grass_plain() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 5)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 5);
        let noise_threshold = feature.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 5);
        assert_eq!(noise_threshold.above_noise, 10);
        assert_eq!(feature.rarity, 1);
        assert_eq!(feature.inner_count, 32);
        assert_eq!(feature.xz_offset.min, -7);
        assert_eq!(feature.xz_offset.max, 7);
        assert_eq!(feature.y_offset.min, -3);
        assert_eq!(feature.y_offset.max, 3);
        assert!(feature.block.lower.is("minecraft:short_grass"));
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:cherry_grove")
        ));
    }

    #[test]
    fn vanilla_noise_configures_normal_mushrooms_and_pumpkins() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let brown_mushroom = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 6)
            .unwrap();
        let red_mushroom = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 7)
            .unwrap();
        let pumpkin = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 8)
            .unwrap();

        assert_eq!(brown_mushroom.step_index, 9);
        assert_eq!(brown_mushroom.rarity, 256);
        assert_eq!(brown_mushroom.inner_count, 96);
        assert!(brown_mushroom.block.lower.is("minecraft:brown_mushroom"));
        assert!(brown_mushroom.required_support.is_none());
        assert!(matches!(
            brown_mushroom.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:plains")
        ));

        assert_eq!(red_mushroom.step_index, 9);
        assert_eq!(red_mushroom.rarity, 512);
        assert_eq!(red_mushroom.inner_count, 96);
        assert!(red_mushroom.block.lower.is("minecraft:red_mushroom"));
        assert!(matches!(
            red_mushroom.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:plains")
        ));

        assert_eq!(pumpkin.step_index, 9);
        assert_eq!(pumpkin.rarity, 300);
        assert_eq!(pumpkin.inner_count, 96);
        assert!(pumpkin.block.lower.is("minecraft:pumpkin"));
        assert_eq!(pumpkin.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            pumpkin.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:snowy_taiga")
        ));
    }

    #[test]
    fn vanilla_noise_configures_dead_bush_patches() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let normal = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 9)
            .unwrap();
        let desert = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 10)
            .unwrap();
        let badlands = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 11)
            .unwrap();

        assert_eq!(normal.step_index, 9);
        assert_eq!(normal.outer_count, 1);
        assert_eq!(normal.rarity, 1);
        assert_eq!(normal.inner_count, 4);
        assert!(normal.block.lower.is("minecraft:dead_bush"));
        assert!(matches!(
            normal.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:swamp")
                    && biomes.contains(&"minecraft:old_growth_pine_taiga")
        ));

        assert_eq!(desert.outer_count, 2);
        assert_eq!(desert.rarity, 1);
        assert!(matches!(
            desert.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DEAD_BUSH_DESERT_BIOMES
        ));

        assert_eq!(badlands.outer_count, 20);
        assert_eq!(badlands.rarity, 1);
        assert!(matches!(
            badlands.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:wooded_badlands")
        ));
    }

    #[test]
    fn vanilla_noise_configures_melon_patches() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let melon = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 12)
            .unwrap();
        let sparse = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 13)
            .unwrap();

        assert_eq!(melon.step_index, 9);
        assert_eq!(melon.outer_count, 1);
        assert_eq!(melon.rarity, 6);
        assert_eq!(melon.inner_count, 64);
        assert!(melon.block.lower.is("minecraft:melon"));
        assert_eq!(melon.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            melon.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:jungle")
                    && biomes.contains(&"minecraft:bamboo_jungle")
        ));

        assert_eq!(sparse.rarity, 64);
        assert_eq!(sparse.inner_count, 64);
        assert_eq!(sparse.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            sparse.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == MELON_SPARSE_PATCH_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_block_column_plants() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let normal_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 14)
            .unwrap();
        let badlands_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 15)
            .unwrap();
        let desert_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 16)
            .unwrap();
        let swamp_cane = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 17)
            .unwrap();
        let desert_cactus = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 18)
            .unwrap();
        let badlands_cactus = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 19)
            .unwrap();

        assert_eq!(normal_cane.step_index, 9);
        assert_eq!(normal_cane.rarity, 6);
        assert_eq!(normal_cane.inner_count, 20);
        assert_eq!(normal_cane.xz_offset.min, -4);
        assert_eq!(normal_cane.xz_offset.max, 4);
        assert_eq!(normal_cane.y_offset.min, 0);
        assert_eq!(normal_cane.y_offset.max, 0);
        assert!(normal_cane.column.block.is("minecraft:sugar_cane"));
        assert_eq!(normal_cane.column.height.min, 2);
        assert_eq!(normal_cane.column.height.max, 4);
        assert!(normal_cane.column.tip.is_none());
        assert!(matches!(
            normal_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:plains")
                    && biomes.contains(&"minecraft:jungle")
        ));

        assert_eq!(badlands_cane.rarity, 5);
        assert!(matches!(
            badlands_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:wooded_badlands")
        ));
        assert_eq!(desert_cane.rarity, 1);
        assert!(matches!(
            desert_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SUGAR_CANE_DESERT_BIOMES
        ));
        assert_eq!(swamp_cane.rarity, 3);
        assert!(matches!(
            swamp_cane.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SUGAR_CANE_SWAMP_BIOMES
        ));

        assert_eq!(desert_cactus.rarity, 6);
        assert_eq!(desert_cactus.inner_count, 10);
        assert_eq!(desert_cactus.xz_offset.min, -7);
        assert_eq!(desert_cactus.y_offset.min, -3);
        assert!(desert_cactus.column.block.is("minecraft:cactus"));
        assert_eq!(desert_cactus.column.height.min, 1);
        assert_eq!(desert_cactus.column.height.max, 3);
        assert!(
            desert_cactus
                .column
                .tip
                .as_ref()
                .is_some_and(|tip| tip.block.is("minecraft:cactus_flower"))
        );
        assert!(matches!(
            desert_cactus.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == CACTUS_DESERT_BIOMES
        ));
        assert_eq!(badlands_cactus.rarity, 13);
        assert!(matches!(
            badlands_cactus.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:eroded_badlands")
        ));
    }

    #[test]
    fn vanilla_noise_configures_additional_grass_patches() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let normal = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 20)
            .unwrap();
        let forest = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 21)
            .unwrap();
        let badlands = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 22)
            .unwrap();
        let savanna = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 23)
            .unwrap();
        let taiga = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 24)
            .unwrap();
        let taiga_2 = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 25)
            .unwrap();
        let jungle = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 26)
            .unwrap();
        let meadow = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 27)
            .unwrap();
        let large_fern = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 28)
            .unwrap();

        assert_eq!(normal.outer_count, 5);
        assert_eq!(normal.inner_count, 32);
        assert!(matches!(
            normal.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:swamp")
                    && biomes.contains(&"minecraft:windswept_savanna")
        ));
        assert_eq!(forest.outer_count, 2);
        assert!(matches!(
            forest.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:pale_garden")
        ));
        assert_eq!(badlands.outer_count, 1);
        assert!(matches!(
            badlands.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:river")
        ));
        assert_eq!(savanna.outer_count, 20);
        assert!(matches!(
            savanna.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == PATCH_GRASS_SAVANNA_BIOMES
        ));
        assert_eq!(taiga.outer_count, 7);
        assert!(matches!(
            &taiga.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.len() == 2
                    && entries[0].0.is("minecraft:short_grass")
                    && entries[0].1 == 1
                    && entries[1].0.is("minecraft:fern")
                    && entries[1].1 == 4
        ));
        assert_eq!(taiga_2.outer_count, 1);
        assert!(matches!(
            taiga_2.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == PATCH_GRASS_TAIGA_2_BIOMES
        ));
        assert_eq!(jungle.outer_count, 25);
        assert!(matches!(
            &jungle.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries[0].0.is("minecraft:short_grass")
                    && entries[0].1 == 3
                    && entries[1].0.is("minecraft:fern")
                    && entries[1].1 == 1
        ));
        assert_eq!(meadow.inner_count, 16);
        assert!(meadow.noise_threshold.is_some());
        assert!(large_fern.block.lower.is("minecraft:large_fern"));
        assert!(large_fern.block.upper.as_ref().is_some_and(|upper| {
            upper.is("minecraft:large_fern")
                && upper
                    .properties
                    .iter()
                    .any(|(name, value)| name == "half" && value == "upper")
        }));
    }

    #[test]
    fn vanilla_noise_configures_dry_grass_and_mushroom_variants() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let desert_dry_grass = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 29)
            .unwrap();
        let badlands_dry_grass = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 30)
            .unwrap();
        let brown_taiga = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 31)
            .unwrap();
        let red_taiga = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 32)
            .unwrap();
        let brown_old_growth = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 33)
            .unwrap();
        let red_old_growth = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 34)
            .unwrap();
        let brown_swamp = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 35)
            .unwrap();
        let red_swamp = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 36)
            .unwrap();

        assert_eq!(desert_dry_grass.rarity, 3);
        assert_eq!(desert_dry_grass.inner_count, 64);
        assert!(matches!(
            &desert_dry_grass.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries[0].0.is("minecraft:short_dry_grass")
                    && entries[0].1 == 1
                    && entries[1].0.is("minecraft:tall_dry_grass")
                    && entries[1].1 == 1
        ));
        assert!(matches!(
            desert_dry_grass.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DRY_GRASS_DESERT_BIOMES
        ));
        assert_eq!(badlands_dry_grass.rarity, 6);
        assert!(matches!(
            badlands_dry_grass.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:badlands")
                    && biomes.contains(&"minecraft:wooded_badlands")
        ));

        assert_eq!(brown_taiga.rarity, 4);
        assert_eq!(brown_taiga.outer_count, 1);
        assert_eq!(red_taiga.rarity, 256);
        assert!(matches!(
            brown_taiga.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:taiga")
                    && biomes.contains(&"minecraft:mushroom_fields")
        ));
        assert_eq!(brown_old_growth.outer_count, 3);
        assert_eq!(brown_old_growth.rarity, 4);
        assert_eq!(red_old_growth.rarity, 171);
        assert!(matches!(
            red_old_growth.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:old_growth_pine_taiga")
                    && biomes.contains(&"minecraft:old_growth_spruce_taiga")
        ));
        assert_eq!(brown_swamp.outer_count, 2);
        assert_eq!(brown_swamp.rarity, 1);
        assert_eq!(red_swamp.rarity, 64);
        assert!(matches!(
            red_swamp.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SWAMP_MUSHROOM_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_flower_variants() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let default = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 37)
            .unwrap();
        let warm = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 38)
            .unwrap();
        let swamp = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 39)
            .unwrap();
        let cherry = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 40)
            .unwrap();
        let pale_garden = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 41)
            .unwrap();

        assert_eq!(default.rarity, 32);
        assert_eq!(default.inner_count, 1);
        assert_eq!(default.xz_offset.min, 0);
        assert_eq!(default.xz_offset.max, 0);
        assert_eq!(default.y_offset.min, 0);
        assert_eq!(default.y_offset.max, 0);
        assert!(matches!(
            &default.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.len() == 2
                    && entries[0].0.is("minecraft:poppy")
                    && entries[0].1 == 2
                    && entries[1].0.is("minecraft:dandelion")
                    && entries[1].1 == 1
        ));
        assert!(matches!(
            default.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:forest")
                    && biomes.contains(&"minecraft:desert")
                    && biomes.contains(&"minecraft:taiga")
        ));

        assert_eq!(warm.rarity, 16);
        assert_eq!(warm.inner_count, 1);
        assert!(matches!(
            warm.biome_filter,
            FeatureBiomeFilter::Include(biomes)
                if biomes.contains(&"minecraft:jungle")
                    && biomes.contains(&"minecraft:savanna")
                    && biomes.contains(&"minecraft:sparse_jungle")
        ));

        assert_eq!(swamp.rarity, 32);
        assert_eq!(swamp.inner_count, 64);
        assert_eq!(swamp.xz_offset.min, -6);
        assert_eq!(swamp.xz_offset.max, 6);
        assert_eq!(swamp.y_offset.min, -2);
        assert_eq!(swamp.y_offset.max, 2);
        assert!(swamp.block.lower.is("minecraft:blue_orchid"));
        assert!(matches!(
            swamp.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == FLOWER_SWAMP_BIOMES
        ));

        let noise_threshold = cherry.noise_threshold.as_ref().unwrap();
        assert_eq!(noise_threshold.noise_level, -0.8);
        assert_eq!(noise_threshold.below_noise, 5);
        assert_eq!(noise_threshold.above_noise, 10);
        assert_eq!(cherry.rarity, 1);
        assert_eq!(cherry.inner_count, 96);
        assert_eq!(cherry.xz_offset.min, -6);
        assert_eq!(cherry.xz_offset.max, 6);
        assert!(matches!(
            &cherry.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.len() == 16
                    && entries.iter().all(|(layer, weight)| {
                        layer.is("minecraft:pink_petals")
                            && *weight == 1
                            && layer.properties.iter().any(|(name, value)| {
                                name == "facing"
                                    && matches!(value.as_str(), "north" | "east" | "south" | "west")
                            })
                            && layer.properties.iter().any(|(name, value)| {
                                name == "flower_amount"
                                    && matches!(value.as_str(), "1" | "2" | "3" | "4")
                            })
                    })
        ));
        assert!(matches!(
            cherry.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == FLOWER_CHERRY_BIOMES
        ));

        assert_eq!(pale_garden.rarity, 32);
        assert_eq!(pale_garden.inner_count, 1);
        assert!(pale_garden.block.lower.is("minecraft:closed_eyeblossom"));
        assert!(matches!(
            pale_garden.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == FLOWER_PALE_GARDEN_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_trees_plains() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 3)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 3);
        assert_eq!(feature.surface_water_depth, 0);
        assert_eq!(feature.count.entries, vec![(0, 19), (1, 1)]);
        assert_eq!(feature.count.total_weight, 20);
        assert_eq!(feature.config.variants.len(), 2);
        assert_eq!(feature.config.variants[0].chance, 0.333_333_34);
        assert_eq!(
            feature.config.variants[0].placement,
            TreePlacementKind::Standing
        );
        assert_eq!(feature.config.variants[1].chance, 0.0125);
        assert_eq!(
            feature.config.variants[1].placement,
            TreePlacementKind::Fallen
        );
        assert_eq!(feature.config.default_tree.base_height, 4);
        assert_eq!(feature.config.default_tree.height_rand_a, 2);
        assert_eq!(feature.config.default_tree.height_rand_b, 0);
        assert_eq!(feature.config.default_tree.foliage_height, 3);
        assert_eq!(feature.config.default_tree.foliage_radius, 2);
        assert_eq!(feature.config.default_tree.beehive_probability, 0.05);
        assert_eq!(feature.config.default_tree.fallen_min_length, 4);
        assert_eq!(feature.config.default_tree.fallen_max_length, 7);
        assert!(feature.config.default_tree.trunk.is("minecraft:oak_log"));
        assert!(
            feature
                .config
                .default_tree
                .leaves
                .is("minecraft:oak_leaves")
        );
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == PLAINS_TREE_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_birch_trees() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let birch = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 42)
            .unwrap();
        let tall_birch = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 43)
            .unwrap();

        assert_eq!(birch.step_index, 9);
        assert_eq!(birch.count.entries, vec![(10, 9), (11, 1)]);
        assert_eq!(birch.count.total_weight, 10);
        assert_eq!(birch.config.variants.len(), 1);
        assert_eq!(birch.config.variants[0].chance, 0.0125);
        assert_eq!(
            birch.config.variants[0].placement,
            TreePlacementKind::Fallen
        );
        assert_eq!(birch.config.default_tree.base_height, 5);
        assert_eq!(birch.config.default_tree.height_rand_a, 2);
        assert_eq!(birch.config.default_tree.height_rand_b, 0);
        assert_eq!(birch.config.default_tree.beehive_probability, 0.002);
        assert_eq!(birch.config.default_tree.fallen_min_length, 5);
        assert_eq!(birch.config.default_tree.fallen_max_length, 8);
        assert!(birch.config.default_tree.trunk.is("minecraft:birch_log"));
        assert!(
            birch
                .config
                .default_tree
                .leaves
                .is("minecraft:birch_leaves")
        );
        assert!(matches!(
            birch.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == BIRCH_TREE_BIOMES
        ));

        assert_eq!(tall_birch.step_index, 9);
        assert_eq!(tall_birch.count.entries, vec![(10, 9), (11, 1)]);
        assert_eq!(tall_birch.config.variants.len(), 3);
        assert_eq!(tall_birch.config.variants[0].chance, 0.00625);
        assert_eq!(
            tall_birch.config.variants[0].placement,
            TreePlacementKind::Fallen
        );
        assert_eq!(tall_birch.config.variants[0].tree.height_rand_b, 6);
        assert_eq!(tall_birch.config.variants[0].tree.fallen_max_length, 15);
        assert_eq!(tall_birch.config.variants[1].chance, 0.5);
        assert_eq!(
            tall_birch.config.variants[1].placement,
            TreePlacementKind::Standing
        );
        assert_eq!(tall_birch.config.variants[1].tree.height_rand_b, 6);
        assert_eq!(tall_birch.config.variants[2].chance, 0.0125);
        assert_eq!(
            tall_birch.config.variants[2].placement,
            TreePlacementKind::Fallen
        );
        assert!(matches!(
            tall_birch.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == TALL_BIRCH_TREE_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_spruce_trees() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let taiga = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 44)
            .unwrap();
        let snowy = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 45)
            .unwrap();

        assert_eq!(taiga.step_index, 9);
        assert_eq!(taiga.count.entries, vec![(10, 9), (11, 1)]);
        assert_eq!(taiga.count.total_weight, 10);
        assert_eq!(taiga.config.variants.len(), 2);
        assert_eq!(taiga.config.variants[0].chance, 0.333_333_34);
        assert_eq!(
            taiga.config.variants[0].placement,
            TreePlacementKind::Standing
        );
        assert_eq!(taiga.config.variants[0].tree.base_height, 6);
        assert_eq!(taiga.config.variants[0].tree.height_rand_a, 4);
        assert_eq!(taiga.config.variants[0].tree.height_rand_b, 0);
        assert_eq!(taiga.config.variants[1].chance, 0.0125);
        assert_eq!(
            taiga.config.variants[1].placement,
            TreePlacementKind::Fallen
        );
        assert_eq!(taiga.config.default_tree.base_height, 5);
        assert_eq!(taiga.config.default_tree.height_rand_a, 2);
        assert_eq!(taiga.config.default_tree.height_rand_b, 1);
        assert_eq!(taiga.config.default_tree.beehive_probability, 0.0);
        assert_eq!(taiga.config.default_tree.fallen_min_length, 6);
        assert_eq!(taiga.config.default_tree.fallen_max_length, 10);
        assert!(taiga.config.default_tree.trunk.is("minecraft:spruce_log"));
        assert!(
            taiga
                .config
                .default_tree
                .leaves
                .is("minecraft:spruce_leaves")
        );
        assert!(matches!(
            taiga.config.default_tree.foliage,
            TreeFoliageConfig::Spruce {
                radius: UniformInt { min: 2, max: 3 },
                offset: UniformInt { min: 0, max: 2 },
                trunk_height: UniformInt { min: 1, max: 2 },
            }
        ));
        assert!(matches!(
            taiga.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == TAIGA_TREE_BIOMES
        ));

        assert_eq!(snowy.step_index, 9);
        assert_eq!(snowy.count.entries, vec![(0, 9), (1, 1)]);
        assert_eq!(snowy.count.total_weight, 10);
        assert_eq!(snowy.config.variants.len(), 1);
        assert_eq!(snowy.config.variants[0].chance, 0.0125);
        assert!(snowy.config.default_tree.trunk.is("minecraft:spruce_log"));
        assert!(matches!(
            snowy.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SNOWY_TREE_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_savanna_trees() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let savanna = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 46)
            .unwrap();
        let windswept = settings
            .ore_features
            .trees
            .iter()
            .find(|feature| feature.feature_index == 47)
            .unwrap();

        assert_eq!(savanna.step_index, 9);
        assert_eq!(savanna.count.entries, vec![(1, 9), (2, 1)]);
        assert_eq!(savanna.count.total_weight, 10);
        assert_eq!(savanna.config.variants.len(), 2);
        assert_eq!(savanna.config.variants[0].chance, 0.8);
        assert_eq!(
            savanna.config.variants[0].placement,
            TreePlacementKind::Standing
        );
        assert_eq!(
            savanna.config.variants[0].tree.trunk_placer,
            TreeTrunkConfig::Forking
        );
        assert_eq!(savanna.config.variants[0].tree.base_height, 5);
        assert_eq!(savanna.config.variants[0].tree.height_rand_a, 2);
        assert_eq!(savanna.config.variants[0].tree.height_rand_b, 2);
        assert!(savanna.config.variants[0].tree.trunk.is("minecraft:acacia_log"));
        assert!(
            savanna.config.variants[0]
                .tree
                .leaves
                .is("minecraft:acacia_leaves")
        );
        assert!(matches!(
            savanna.config.variants[0].tree.foliage,
            TreeFoliageConfig::Acacia {
                radius: UniformInt { min: 2, max: 2 },
                offset: UniformInt { min: 0, max: 0 },
            }
        ));
        assert_eq!(savanna.config.variants[1].chance, 0.0125);
        assert_eq!(
            savanna.config.variants[1].placement,
            TreePlacementKind::Fallen
        );
        assert!(savanna.config.default_tree.trunk.is("minecraft:oak_log"));
        assert_eq!(savanna.config.default_tree.beehive_probability, 0.0);
        assert!(matches!(
            savanna.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SAVANNA_TREE_BIOMES
        ));

        assert_eq!(windswept.count.entries, vec![(2, 9), (3, 1)]);
        assert_eq!(windswept.count.total_weight, 10);
        assert!(matches!(
            windswept.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == WINDSWEPT_SAVANNA_TREE_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_remaining_overworld_tree_features() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);

        assert_eq!(settings.ore_features.trees.len(), 25);

        let dark_forest = tree_feature(&settings, 48);
        assert_tree_feature(dark_forest, &[(16, 1)], 0, DARK_FOREST_TREE_BIOMES);
        assert_eq!(dark_forest.config.variants.len(), 5);
        assert!(dark_forest.config.variants[0]
            .tree
            .trunk
            .is("minecraft:dark_oak_log"));
        assert_eq!(dark_forest.config.variants[0].chance, 0.666_666_7);

        let pale_garden = tree_feature(&settings, 49);
        assert_tree_feature(pale_garden, &[(16, 1)], 0, PALE_GARDEN_TREE_BIOMES);
        assert!(pale_garden
            .config
            .default_tree
            .trunk
            .is("minecraft:pale_oak_log"));
        assert_eq!(pale_garden.config.default_tree.trunk_placer, TreeTrunkConfig::Giant);

        let flower_forest = tree_feature(&settings, 50);
        assert_tree_feature(
            flower_forest,
            &[(6, 9), (7, 1)],
            0,
            FLOWER_FOREST_TREE_BIOMES,
        );
        assert_eq!(flower_forest.config.variants.len(), 3);
        assert_eq!(flower_forest.config.default_tree.beehive_probability, 0.02);

        let meadow = tree_feature(&settings, 51);
        assert_tree_feature(meadow, &[(0, 99), (1, 1)], 0, MEADOW_TREE_BIOMES);
        assert!(meadow
            .config
            .default_tree
            .trunk
            .is("minecraft:birch_log"));
        assert_eq!(meadow.config.default_tree.height_rand_b, 6);
        assert_eq!(meadow.config.variants[0].tree.beehive_probability, 1.0);

        let cherry = tree_feature(&settings, 52);
        assert_tree_feature(cherry, &[(10, 9), (11, 1)], 0, CHERRY_TREE_BIOMES);
        assert!(cherry.config.default_tree.trunk.is("minecraft:cherry_log"));
        assert!(cherry
            .config
            .default_tree
            .leaves
            .is("minecraft:cherry_leaves"));
        assert_eq!(cherry.config.default_tree.beehive_probability, 0.05);

        let grove = tree_feature(&settings, 53);
        assert_tree_feature(grove, &[(10, 9), (11, 1)], 0, GROVE_TREE_BIOMES);
        assert!(grove.config.default_tree.trunk.is("minecraft:spruce_log"));
        assert_eq!(grove.config.variants[0].tree.base_height, 6);

        let badlands = tree_feature(&settings, 54);
        assert_tree_feature(badlands, &[(5, 9), (6, 1)], 0, BADLANDS_TREE_BIOMES);
        assert!(badlands.config.default_tree.trunk.is("minecraft:oak_log"));

        let swamp = tree_feature(&settings, 55);
        assert_tree_feature(swamp, &[(2, 9), (3, 1)], 2, SWAMP_TREE_BIOMES);
        assert_eq!(swamp.config.default_tree.base_height, 5);
        assert_eq!(swamp.config.default_tree.foliage_radius, 3);

        let windswept_hills = tree_feature(&settings, 56);
        assert_tree_feature(
            windswept_hills,
            &[(0, 9), (1, 1)],
            0,
            WINDSWEPT_HILLS_TREE_BIOMES,
        );
        assert_eq!(windswept_hills.config.variants.len(), 4);
        assert!(windswept_hills.config.variants[1]
            .tree
            .trunk
            .is("minecraft:spruce_log"));

        let windswept_forest = tree_feature(&settings, 57);
        assert_tree_feature(
            windswept_forest,
            &[(3, 9), (4, 1)],
            0,
            WINDSWEPT_FOREST_TREE_BIOMES,
        );
        assert_eq!(windswept_forest.config.variants.len(), 4);

        let water = tree_feature(&settings, 58);
        assert_tree_feature(water, &[(0, 9), (1, 1)], 0, WATER_TREE_BIOMES);
        assert_eq!(water.config.variants.len(), 1);

        let forest = tree_feature(&settings, 59);
        assert_tree_feature(
            forest,
            &[(10, 9), (11, 1)],
            0,
            BIRCH_AND_OAK_LEAF_LITTER_TREE_BIOMES,
        );
        assert_eq!(forest.config.variants.len(), 4);

        let sparse_jungle = tree_feature(&settings, 60);
        assert_tree_feature(
            sparse_jungle,
            &[(2, 9), (3, 1)],
            0,
            SPARSE_JUNGLE_TREE_BIOMES,
        );
        assert!(sparse_jungle
            .config
            .default_tree
            .trunk
            .is("minecraft:jungle_log"));
        assert!(sparse_jungle.config.variants[1]
            .tree
            .leaves
            .is("minecraft:oak_leaves"));

        let old_growth_spruce = tree_feature(&settings, 61);
        assert_tree_feature(
            old_growth_spruce,
            &[(10, 9), (11, 1)],
            0,
            OLD_GROWTH_SPRUCE_TAIGA_TREE_BIOMES,
        );
        assert_eq!(
            old_growth_spruce.config.variants[0].tree.trunk_placer,
            TreeTrunkConfig::Giant
        );

        let old_growth_pine = tree_feature(&settings, 62);
        assert_tree_feature(
            old_growth_pine,
            &[(10, 9), (11, 1)],
            0,
            OLD_GROWTH_PINE_TAIGA_TREE_BIOMES,
        );
        assert_eq!(old_growth_pine.config.variants.len(), 4);

        let jungle = tree_feature(&settings, 63);
        assert_tree_feature(jungle, &[(50, 9), (51, 1)], 0, JUNGLE_TREE_BIOMES);
        assert!(jungle.config.default_tree.trunk.is("minecraft:jungle_log"));
        assert_eq!(
            jungle.config.variants[2].tree.trunk_placer,
            TreeTrunkConfig::Giant
        );

        let bamboo = tree_feature(&settings, 64);
        assert_tree_feature(
            bamboo,
            &[(30, 9), (31, 1)],
            0,
            BAMBOO_JUNGLE_TREE_BIOMES,
        );
        assert_eq!(bamboo.config.variants.len(), 3);
        assert_eq!(
            bamboo.config.variants[2].tree.trunk_placer,
            TreeTrunkConfig::Giant
        );

        let mangrove = tree_feature(&settings, 65);
        assert_tree_feature(mangrove, &[(25, 1)], 5, MANGROVE_TREE_BIOMES);
        assert!(mangrove
            .config
            .default_tree
            .trunk
            .is("minecraft:mangrove_log"));
        assert_eq!(mangrove.config.variants[0].tree.base_height, 8);
    }

    #[test]
    fn vanilla_noise_configures_freeze_top_layer() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = &settings.ore_features.freeze_top_layer;

        assert_eq!(feature.step_index, 10);
        assert_eq!(feature.feature_index, 0);
        assert!(feature.snow_layer.is("minecraft:snow"));
    }

    #[test]
    fn vanilla_noise_ore_biome_filter_skips_disallowed_positions() {
        const MISSING_BIOMES: &[&str] = &["minecraft:missing_biome"];

        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| stone.clone())
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let ore = OreFeatureConfig::new(
            32,
            0.0,
            "minecraft:emerald_ore",
            "minecraft:deepslate_emerald_ore",
        );
        let feature = PlacedOreFeature::new(
            0,
            OrePlacementCount::Constant(64),
            OreHeight::Uniform(HeightAnchor::Absolute(48), HeightAnchor::Absolute(48)),
            ore,
        )
        .with_biome_filter(FeatureBiomeFilter::Include(MISSING_BIOMES));
        let mut random = FeatureRandom::new(12345);

        feature.place(&settings, 0, 0, &mut chunk, &mut random);

        assert!(chunk.columns.iter().all(|column| {
            column
                .blocks
                .iter()
                .all(|layer| layer.block.as_ref() != "minecraft:emerald_ore")
        }));
    }

    #[test]
    fn vanilla_noise_soft_disk_replaces_submerged_floor() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let dirt = BlockLayer::new("minecraft:dirt");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| match y {
                        i32::MIN..=61 => stone.clone(),
                        62 => dirt.clone(),
                        63 => water.clone(),
                        _ => air.clone(),
                    })
                    .collect(),
                first_available_height: 128,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        PlacedDiskFeature::sand(0).place(&settings, 0, 0, &mut chunk, &mut random);

        assert_eq!(chunk.ocean_floor_wg_height(0, 0, settings.min_y), 63);
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.block.as_ref() == "minecraft:sand")
        }));
    }

    #[test]
    fn vanilla_noise_spring_places_when_rock_and_hole_counts_match() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| stone.clone())
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        chunk.set_layer(8, 64, 8, settings.min_y, air.clone());
        chunk.set_layer(9, 64, 8, settings.min_y, air);
        let spring = PlacedSpringFeature::water(0);

        assert!(spring.config.try_place(&settings, &mut chunk, 8, 64, 8));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:water")
        );
    }

    #[test]
    fn vanilla_noise_lava_lake_carves_cavity_and_fluid() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| stone.clone())
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        assert!(LakeFeatureConfig::lava().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            80,
            8,
        ));

        let mut has_lava = false;
        let mut has_cave_air = false;
        for column in &chunk.columns {
            for layer in &column.blocks {
                has_lava |= layer.is("minecraft:lava");
                has_cave_air |= layer.is("minecraft:cave_air");
            }
        }

        assert!(has_lava);
        assert!(has_cave_air);
    }

    #[test]
    fn vanilla_noise_monster_room_places_shell_spawner_and_chest_entities() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let cave_air = BlockLayer::new("minecraft:cave_air");
        let mut placed = None;

        for seed in 0..10_000 {
            let columns = (0..HEIGHTMAP_ENTRY_COUNT)
                .map(|_| NoiseColumnBlocks {
                    blocks: (settings.min_y..settings.min_y + settings.height)
                        .map(|_| stone.clone())
                        .collect(),
                    first_available_height: settings.height,
                })
                .collect();
            let mut chunk = NoiseChunkBlocks {
                columns,
                biomes: Vec::new(),
                block_entities: Vec::new(),
            };
            for (x, z) in [
                (4, 8),
                (5, 8),
                (11, 8),
                (12, 8),
                (8, 4),
                (8, 5),
                (8, 11),
                (8, 12),
            ] {
                for y in 64..=65 {
                    chunk.set_layer(x, y, z, settings.min_y, cave_air.clone());
                }
            }

            let mut random = FeatureRandom::new(seed);
            if MonsterRoomFeatureConfig::new().place(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                64,
                8,
            ) && chunk
                .block_entities
                .iter()
                .any(|entity| entity.entity_type == CHEST_BLOCK_ENTITY_TYPE_ID)
            {
                placed = Some(chunk);
                break;
            }
        }

        let chunk = placed.expect("test seed range should include a room with a chest");

        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:spawner")
        );
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:mossy_cobblestone"))
        }));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID && entity.position == (8, 64, 8)
        }));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == CHEST_BLOCK_ENTITY_TYPE_ID
                && matches!(&entity.nbt, Tag::Compound(fields) if fields.contains_key("LootTable"))
        }));
    }

    #[test]
    fn vanilla_noise_glow_lichen_places_on_air_or_water_next_to_rock() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let water = BlockLayer::new("minecraft:water");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 50 { stone.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 51 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        chunk.set_layer(8, 48, 8, settings.min_y, water);
        let mut random = FeatureRandom::new(12345);

        assert!(MultifaceGrowthFeatureConfig::glow_lichen().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            48,
            8,
        ));

        let layer = chunk.layer(8, 48, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:glow_lichen"));
        assert!(
            layer
                .properties
                .iter()
                .any(|(name, value)| name == "waterlogged" && value == "true")
        );
        assert!(layer.properties.iter().any(|(name, value)| {
            matches!(name.as_str(), "up" | "north" | "south" | "east" | "west") && value == "true"
        }));
    }

    #[test]
    fn vanilla_noise_glow_lichen_respects_ocean_floor_threshold() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 80 { stone.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 81 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let feature = PlacedMultifaceGrowthFeature::glow_lichen(0);
        let mut random = FeatureRandom::new(12345);

        feature.place(&settings, 0, 0, &mut chunk, &mut random);

        for column in &chunk.columns {
            for (index, layer) in column.blocks.iter().enumerate() {
                if layer.is("minecraft:glow_lichen") {
                    let y = settings.min_y + index as i32;
                    assert!(y <= 68);
                }
            }
        }
    }

    #[test]
    fn vanilla_noise_patch_tall_grass_places_double_plant_on_vegetation_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::tall_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:tall_grass"));
        assert!(
            lower
                .properties
                .iter()
                .any(|(name, value)| { name == "half" && value == "lower" })
        );
        assert!(upper.is("minecraft:tall_grass"));
        assert!(
            upper
                .properties
                .iter()
                .any(|(name, value)| { name == "half" && value == "upper" })
        );
    }

    #[test]
    fn vanilla_noise_patch_tall_grass_requires_air_and_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| air.clone())
                    .collect(),
                first_available_height: 0,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);
        chunk.set_layer(8, 64, 8, settings.min_y, stone.clone());
        assert!(!SimpleVegetationBlock::tall_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        chunk.set_layer(8, 64, 8, settings.min_y, grass_block);
        chunk.set_layer(8, 66, 8, settings.min_y, stone);
        assert!(!SimpleVegetationBlock::tall_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));
    }

    #[test]
    fn vanilla_noise_patch_bush_places_single_vegetation_block() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::single("minecraft:bush").place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:bush")
        );
        assert!(
            chunk
                .layer(8, 66, 8, settings.min_y)
                .is_some_and(|layer| layer.is_air)
        );
    }

    #[test]
    fn vanilla_noise_flower_plains_places_noise_selected_flower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::plains_flower().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let flower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(
            PLAINS_FLOWER_LOW_BLOCKS.contains(&flower.block.as_ref())
                || PLAINS_FLOWER_HIGH_BLOCKS.contains(&flower.block.as_ref())
                || flower.is("minecraft:dandelion")
        );
    }

    #[test]
    fn vanilla_noise_patch_grass_plain_places_short_grass() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(
            SimpleVegetationBlock::single("minecraft:short_grass").place_at(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                65,
                8,
            )
        );

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:short_grass")
        );
    }

    #[test]
    fn vanilla_noise_pumpkin_patch_requires_grass_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let dirt = BlockLayer::new("minecraft:dirt");
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| air.clone())
                    .collect(),
                first_available_height: 0,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let pumpkin = PlacedSimpleVegetationFeature::patch_pumpkin(0);

        chunk.set_layer(8, 64, 8, settings.min_y, dirt);
        assert!(!pumpkin.has_required_support(&chunk, 0, 0, 8, 64, 8, settings.min_y));

        chunk.set_layer(8, 64, 8, settings.min_y, grass_block);
        assert!(pumpkin.has_required_support(&chunk, 0, 0, 8, 64, 8, settings.min_y));
    }

    #[test]
    fn vanilla_noise_melon_patch_uses_grass_support() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::single("minecraft:melon").place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:melon")
        );
        assert!(
            PlacedSimpleVegetationFeature::patch_melon(0, 6).has_required_support(
                &chunk,
                0,
                0,
                8,
                64,
                8,
                settings.min_y
            )
        );
    }

    #[test]
    fn vanilla_noise_normal_mushroom_places_single_block() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(
            SimpleVegetationBlock::single("minecraft:brown_mushroom").place_at(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                65,
                8,
            )
        );

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:brown_mushroom")
        );
    }

    #[test]
    fn vanilla_noise_dead_bush_uses_dead_bush_support_rules() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::dead_bush().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert_eq!(
            chunk
                .layer(8, 65, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dead_bush")
        );
        assert!(!supports_vegetation_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
        assert!(supports_dead_bush_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
    }

    #[test]
    fn vanilla_noise_weighted_grass_selects_configured_blocks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::taiga_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let layer = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:short_grass") || layer.is("minecraft:fern"));
    }

    #[test]
    fn vanilla_noise_large_fern_places_double_plant() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::large_fern().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:large_fern"));
        assert!(
            lower
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "lower")
        );
        assert!(upper.is("minecraft:large_fern"));
        assert!(
            upper
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "upper")
        );
    }

    #[test]
    fn vanilla_noise_dry_grass_uses_dry_vegetation_support_rules() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let red_sand = BlockLayer::new("minecraft:red_sand");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            red_sand.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::dry_grass().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let layer = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:short_dry_grass") || layer.is("minecraft:tall_dry_grass"));
        assert!(!supports_vegetation_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
        assert!(supports_dry_vegetation_layer(
            chunk.layer(8, 64, 8, settings.min_y).unwrap()
        ));
    }

    #[test]
    fn vanilla_noise_default_flower_places_weighted_flower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::default_flower().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let flower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(flower.is("minecraft:poppy") || flower.is("minecraft:dandelion"));
    }

    #[test]
    fn vanilla_noise_cherry_flower_places_pink_petals_state() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::cherry_flower().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let flower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        assert!(flower.is("minecraft:pink_petals"));
        assert!(flower.properties.iter().any(|(name, value)| {
            name == "facing" && matches!(value.as_str(), "north" | "east" | "south" | "west")
        }));
        assert!(flower.properties.iter().any(|(name, value)| {
            name == "flower_amount" && matches!(value.as_str(), "1" | "2" | "3" | "4")
        }));
    }

    #[test]
    fn vanilla_noise_sugar_cane_requires_adjacent_water() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };

        assert!(!BlockColumnSupport::SugarCane.allows_at_world(
            &chunk,
            0,
            0,
            8,
            65,
            8,
            settings.min_y
        ));

        chunk.set_layer(9, 64, 8, settings.min_y, water);
        assert!(BlockColumnSupport::SugarCane.allows_at_world(
            &chunk,
            0,
            0,
            8,
            65,
            8,
            settings.min_y
        ));
    }

    #[test]
    fn vanilla_noise_sugar_cane_places_column() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        chunk.set_layer(9, 64, 8, settings.min_y, water);
        let mut random = FeatureRandom::new(1);

        assert!(BlockColumnFeatureConfig::sugar_cane().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let placed = (65..=68)
            .filter(|y| {
                chunk
                    .layer(8, *y, 8, settings.min_y)
                    .is_some_and(|layer| layer.is("minecraft:sugar_cane"))
            })
            .count();
        assert!((2..=4).contains(&placed));
    }

    #[test]
    fn vanilla_noise_cactus_requires_open_sides() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };

        assert!(BlockColumnSupport::Cactus.allows_at_world(&chunk, 0, 0, 8, 65, 8, settings.min_y));

        chunk.set_layer(9, 65, 8, settings.min_y, stone);
        assert!(!BlockColumnSupport::Cactus.allows_at_world(
            &chunk,
            0,
            0,
            8,
            65,
            8,
            settings.min_y
        ));
    }

    #[test]
    fn vanilla_noise_cactus_places_column_and_optional_flower() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let sand = BlockLayer::new("minecraft:sand");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| if y <= 64 { sand.clone() } else { air.clone() })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(BlockColumnFeatureConfig::cactus().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:cactus"))
        }));
        assert!(
            !chunk
                .layer(8, 65, 8, settings.min_y)
                .is_some_and(|layer| layer.is_air)
        );
    }

    #[test]
    fn vanilla_noise_sunflower_places_double_plant() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(1);

        assert!(SimpleVegetationBlock::sunflower().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:sunflower"));
        assert!(
            lower
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "lower")
        );
        assert!(upper.is("minecraft:sunflower"));
        assert!(
            upper
                .properties
                .iter()
                .any(|(name, value)| name == "half" && value == "upper")
        );
    }

    #[test]
    fn vanilla_noise_weighted_double_flowers_place_upper_half() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let flower = SimpleVegetationBlock::single("minecraft:rose_bush");

        assert!(flower.place_selected(
            &settings,
            0,
            0,
            &mut chunk,
            BlockLayer::new("minecraft:rose_bush"),
            8,
            65,
            8,
        ));

        let lower = chunk.layer(8, 65, 8, settings.min_y).unwrap();
        let upper = chunk.layer(8, 66, 8, settings.min_y).unwrap();
        assert!(lower.is("minecraft:rose_bush"));
        assert!(lower
            .properties
            .iter()
            .any(|(name, value)| name == "half" && value == "lower"));
        assert!(upper.is("minecraft:rose_bush"));
        assert!(upper
            .properties
            .iter()
            .any(|(name, value)| name == "half" && value == "upper"));
    }

    #[test]
    fn vanilla_noise_patch_tall_grass_uses_biome_info_noise_threshold() {
        let placement = NoiseThresholdCount {
            noise_level: -0.8,
            below_noise: 0,
            above_noise: 7,
        };

        assert_eq!(placement.sample(0, 0), 7);
        assert_eq!(placement.sample(-2_000_000, -1_993_000), 0);
    }

    #[test]
    fn vanilla_noise_trees_plains_oak_places_logs_and_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        assert!(OakTreeConfig::oak_bees_005().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:oak_log"))
        }));
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:oak_leaves"))
        }));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dirt")
        );
    }

    #[test]
    fn vanilla_noise_trees_birch_places_logs_and_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let mut random = FeatureRandom::new(12345);

        assert!(OakTreeConfig::birch_bees_0002().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:birch_log"))
        }));
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:birch_leaves"))
        }));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dirt")
        );
    }

    #[test]
    fn vanilla_noise_trees_spruce_places_logs_and_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let mut random = FeatureRandom::new(12345);

        assert!(OakTreeConfig::spruce().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:spruce_log"))
        }));
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:spruce_leaves"))
        }));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dirt")
        );
    }

    #[test]
    fn vanilla_noise_trees_acacia_places_logs_and_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let mut random = FeatureRandom::new(12345);

        assert!(OakTreeConfig::acacia().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:acacia_log"))
        }));
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:acacia_leaves"))
        }));
        assert_eq!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:dirt")
        );
    }

    #[test]
    fn vanilla_noise_new_tree_types_place_logs_and_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);

        for (tree, log, leaves) in [
            (
                OakTreeConfig::dark_oak(),
                "minecraft:dark_oak_log",
                "minecraft:dark_oak_leaves",
            ),
            (
                OakTreeConfig::pale_oak(),
                "minecraft:pale_oak_log",
                "minecraft:pale_oak_leaves",
            ),
            (
                OakTreeConfig::cherry_bees_005(),
                "minecraft:cherry_log",
                "minecraft:cherry_leaves",
            ),
            (
                OakTreeConfig::jungle_tree(),
                "minecraft:jungle_log",
                "minecraft:jungle_leaves",
            ),
            (
                OakTreeConfig::mega_jungle_tree(),
                "minecraft:jungle_log",
                "minecraft:jungle_leaves",
            ),
            (
                OakTreeConfig::mangrove(),
                "minecraft:mangrove_log",
                "minecraft:mangrove_leaves",
            ),
        ] {
            let mut chunk = grass_surface_test_chunk(&settings, 64);
            let mut random = FeatureRandom::new(12345);

            assert!(tree.place(
                &settings,
                0,
                0,
                &mut chunk,
                &mut random,
                8,
                65,
                8,
            ));
            assert!(chunk_contains_block(&chunk, log), "{log}");
            assert!(chunk_contains_block(&chunk, leaves), "{leaves}");
        }
    }

    #[test]
    fn vanilla_noise_fallen_jungle_places_horizontal_jungle_log() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let mut random = FeatureRandom::new(12345);

        assert!(OakTreeConfig::fallen_jungle().place_fallen(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                layer.is("minecraft:jungle_log")
                    && layer
                        .properties
                        .iter()
                        .any(|(name, value)| name == "axis" && value != "y")
            })
        }));
    }

    #[test]
    fn vanilla_noise_fallen_tree_does_not_use_leaf_canopy_as_ground() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let leaves = BlockLayer::new("minecraft:oak_leaves");
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_layer(x, 65, z, settings.min_y, leaves.clone());
            }
        }
        let mut random = FeatureRandom::new(12345);

        assert!(!OakTreeConfig::fallen_jungle().place_fallen(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            65,
            8,
        ));

        assert!(!chunk_contains_block(&chunk, "minecraft:jungle_log"));
        for column in &chunk.columns {
            assert!(!column.blocks.iter().enumerate().any(|(index, layer)| {
                settings.min_y + index as i32 == 66 && layer.is("minecraft:jungle_log")
            }));
        }
    }

    #[test]
    fn vanilla_noise_tree_without_local_trunk_does_not_place_detached_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 64);
        let mut random = FeatureRandom::new(12345);

        assert!(!OakTreeConfig::oak().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            -1,
            65,
            8,
        ));
        assert!(!chunk_contains_block(&chunk, "minecraft:oak_leaves"));
    }

    #[test]
    fn vanilla_noise_tree_spillover_places_neighbor_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut source = grass_surface_test_chunk(&settings, 64);
        let mut target = grass_surface_test_chunk(&settings, 64);
        let tree = OakTreeConfig::oak();
        let mut random = FeatureRandom::new(12345);
        let mut replay_random = random.clone();

        assert!(tree.place(&settings, 0, 0, &mut source, &mut random, 15, 65, 8));
        assert!(tree.place_spillover(
            &settings,
            16,
            0,
            &mut target,
            &mut replay_random,
            15,
            65,
            8,
        ));

        assert!(chunk_contains_block(&target, "minecraft:oak_leaves"));
        assert!(!chunk_contains_block(&target, "minecraft:oak_log"));
    }

    #[test]
    fn vanilla_noise_seed0_chunk_minus5_10_receives_neighbor_tree_spillover() {
        let settings = NoiseSettings::overworld(0, vanilla_noise::OverworldNoiseKind::Default);
        let without_spillover =
            generate_noise_chunk_without_neighbor_tree_spillover(&settings, -5, 10);
        let with_spillover = settings.generate_chunk(-5, 10);
        let without_count = boundary_tree_block_count(&without_spillover);
        let with_count = boundary_tree_block_count(&with_spillover);

        assert!(
            with_count > without_count,
            "expected neighbor tree spillover at chunk (-5, 10), got boundary tree blocks without={without_count}, with={with_count}"
        );
    }

    #[test]
    fn vanilla_noise_trees_plains_can_place_beehive_entity() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let grass_block = BlockLayer::new("minecraft:grass_block");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= 64 {
                            grass_block.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: 65 - settings.min_y,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut tree = OakTreeConfig::oak_bees_005();
        tree.beehive_probability = 1.0;
        let mut random = FeatureRandom::new(1);

        assert!(tree.place(&settings, 0, 0, &mut chunk, &mut random, 8, 65, 8));

        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:bee_nest"))
        }));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == BEEHIVE_BLOCK_ENTITY_TYPE_ID
                && matches!(&entity.nbt, Tag::Compound(fields) if fields.contains_key("bees"))
        }));
    }

    #[test]
    fn vanilla_noise_block_entities_are_written_to_chunk_packet() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = settings.generate_chunk(0, 0);
        chunk.push_block_entity(
            1,
            64,
            2,
            CHEST_BLOCK_ENTITY_TYPE_ID,
            chest_block_entity_nbt(0),
        );

        let packet_entities = chunk.block_entities_as_packet(0, 0);

        assert_eq!(packet_entities.len(), 1);
        assert_eq!(packet_entities[0].xz, 0x12);
        assert_eq!(packet_entities[0].y, 64);
        assert_eq!(
            packet_entities[0].entity_type,
            VarInt(CHEST_BLOCK_ENTITY_TYPE_ID)
        );
        assert!(matches!(packet_entities[0].nbt, OptionalNbt(Some(_))));
    }

    #[test]
    fn vanilla_noise_amethyst_geode_places_layered_blocks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|_| stone.clone())
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let mut random = FeatureRandom::new(12345);

        assert!(GeodeFeatureConfig::amethyst().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            0,
            8,
        ));

        let mut found = HashSet::new();
        for column in &chunk.columns {
            for layer in &column.blocks {
                match layer.block.as_ref() {
                    "minecraft:smooth_basalt"
                    | "minecraft:calcite"
                    | "minecraft:amethyst_block"
                    | "minecraft:budding_amethyst" => {
                        found.insert(layer.block.clone());
                    }
                    _ => {}
                }
            }
        }

        assert!(found.contains("minecraft:smooth_basalt"));
        assert!(found.contains("minecraft:calcite"));
        assert!(
            found.contains("minecraft:amethyst_block")
                || found.contains("minecraft:budding_amethyst")
        );
    }

    #[test]
    fn vanilla_noise_freeze_top_layer_places_ice_and_snow_in_cold_biomes() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let Some((origin_x, origin_z)) = (-64..=64).step_by(16).find_map(|chunk_x| {
            (-64..=64).step_by(16).find_map(|chunk_z| {
                is_freezing_biome(settings.density.biome(chunk_x, 64, chunk_z))
                    .then_some((chunk_x, chunk_z))
            })
        }) else {
            return;
        };
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let stone = BlockLayer::new("minecraft:stone");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y < 62 {
                            stone.clone()
                        } else if y == 62 {
                            water.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };

        settings
            .ore_features
            .freeze_top_layer
            .place(&settings, origin_x, origin_z, &mut chunk);

        assert_eq!(
            chunk
                .layer(0, 62, 0, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:ice")
        );
        assert_eq!(
            chunk
                .layer(0, 63, 0, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:snow")
        );
    }

    #[test]
    fn vanilla_noise_generates_biome_cells() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let biomes = settings.generate_biomes(0, 0);

        assert_eq!(biomes.len(), section_count() as usize * 64);
        assert!(biomes.iter().all(|biome| biome.starts_with("minecraft:")));
    }

    #[test]
    fn vanilla_noise_loads_large_and_amplified_presets() {
        let large = load_noise_settings("minecraft:large_biomes", 12345).unwrap();
        let amplified = load_noise_settings("minecraft:amplified", 12345).unwrap();
        let default = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let points = [(128, 64, 128), (320, 96, -144), (-512, 140, 384)];
        let differs_from_default = |settings: &NoiseSettings| {
            points.into_iter().any(|(x, y, z)| {
                let profile = settings.density.profile(x, z);
                let default_profile = default.density.profile(x, z);
                settings
                    .density
                    .sample_with_profile(x, y, z, &profile)
                    .to_bits()
                    != default
                        .density
                        .sample_with_profile(x, y, z, &default_profile)
                        .to_bits()
            })
        };

        assert!(differs_from_default(&large));
        assert!(differs_from_default(&amplified));
    }
}
