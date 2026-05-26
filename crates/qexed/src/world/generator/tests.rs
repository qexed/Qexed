#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::*;

    fn grass_surface_test_chunk(settings: &NoiseSettings, surface_y: i32) -> NoiseChunkBlocks {
        surface_test_chunk(settings, surface_y, "minecraft:grass_block")
    }

    fn surface_test_chunk(
        settings: &NoiseSettings,
        surface_y: i32,
        surface_block: &str,
    ) -> NoiseChunkBlocks {
        let surface_block = BlockLayer::new(surface_block);
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= surface_y {
                            surface_block.clone()
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

    fn underwater_test_chunk(
        settings: &NoiseSettings,
        floor_y: i32,
        water_top_y: i32,
        floor_block: &str,
    ) -> NoiseChunkBlocks {
        let floor = BlockLayer::new(floor_block);
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= floor_y {
                            floor.clone()
                        } else if y <= water_top_y {
                            water.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: water_top_y + 1 - settings.min_y,
            })
            .collect();

        NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        }
    }

    fn cave_ceiling_test_chunk(settings: &NoiseSettings, ceiling_y: i32) -> NoiseChunkBlocks {
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y >= ceiling_y {
                            stone.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: settings.height,
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

    fn chunk_contains_property(chunk: &NoiseChunkBlocks, block: &str, name: &str, value: &str) -> bool {
        chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                layer.is(block)
                    && layer
                        .properties
                        .iter()
                        .any(|(property_name, property_value)| {
                            property_name == name && property_value == value
                        })
            })
        })
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
    fn vanilla_noise_does_not_generate_non_overworld_dimensions() {
        let config = WorldConfig {
            generator: WorldGeneratorConfig::VanillaNoise,
            generator_preset: "minecraft:overworld".to_string(),
            seed: 12345,
            ..WorldConfig::default()
        };
        let generator = VanillaNoiseGenerator::from_config(&config);
        let generated = generator
            .generate("minecraft:the_nether", 0, 0, WorldLightAlgorithm::Fast)
            .unwrap();

        assert_eq!(generated.light_dampening, vec![0; CHUNK_DAMPENING_LEN]);
        assert_eq!(generated.packet.data.heightmaps.len(), 3);
        assert_eq!(
            generator.block_state_at(
                "minecraft:the_end",
                &qexed_packet::net_types::Position { x: 0, y: 64, z: 0 }
            ),
            None
        );
        assert!(generator.region_chunk("minecraft:the_nether", 0, 0).unwrap().is_none());
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
    fn vanilla_noise_fills_open_surface_below_sea_level_with_water() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let surface_height = settings.sea_level - 5;
        let layer = settings.layer_at_with_density(
            0,
            settings.sea_level - 1,
            0,
            -1.0,
            surface_height,
            surface_height,
            0,
            None,
        );

        assert!(layer.is("minecraft:water"));
    }

    #[test]
    fn vanilla_noise_keeps_sea_level_surface_as_air() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let surface_height = settings.sea_level - 5;
        let layer = settings.layer_at_with_density(
            0,
            settings.sea_level,
            0,
            -1.0,
            surface_height,
            surface_height,
            0,
            None,
        );

        assert!(layer.is_air);
    }

    #[test]
    fn vanilla_noise_treats_open_surface_water_as_underwater_for_surface_rules() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let surface_height = settings.sea_level - 5;
        let water_height = Some(settings.sea_level - 1);
        let layer = settings.layer_at_with_density(
            0,
            surface_height,
            0,
            1.0,
            surface_height,
            surface_height,
            0,
            water_height,
        );

        assert!(layer.is("minecraft:dirt"));
    }

    #[test]
    fn vanilla_noise_leaves_open_surface_above_sea_level_as_air() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let surface_height = settings.sea_level - 5;
        let layer = settings.layer_at_with_density(
            0,
            settings.sea_level + 1,
            0,
            -1.0,
            surface_height,
            surface_height,
            0,
            None,
        );

        assert!(layer.is_air);
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
    fn vanilla_noise_carver_keeps_surface_water_column_filled() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk =
            underwater_test_chunk(&settings, 60, settings.sea_level - 1, "minecraft:stone");
        let preliminary_surfaces = vec![60; HEIGHTMAP_ENTRY_COUNT];
        let mut has_grass = false;

        assert!(carve_block(
            &settings,
            &mut chunk,
            &preliminary_surfaces,
            8,
            8,
            60,
            8,
            8,
            settings.min_y + 8,
            &mut has_grass,
        ));

        assert!(chunk.layer(8, 60, 8, settings.min_y).unwrap().is("minecraft:water"));
    }

    #[test]
    fn vanilla_noise_carver_floods_blocks_adjacent_to_surface_water() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|column| {
                let x = column % 16;
                NoiseColumnBlocks {
                    blocks: (settings.min_y..settings.min_y + settings.height)
                        .map(|y| {
                            if x == 7 && (61..settings.sea_level).contains(&y) {
                                water.clone()
                            } else if y < settings.sea_level {
                                stone.clone()
                            } else {
                                air.clone()
                            }
                        })
                        .collect(),
                    first_available_height: settings.sea_level - settings.min_y,
                }
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let preliminary_surfaces = vec![settings.sea_level; HEIGHTMAP_ENTRY_COUNT];
        let mut has_grass = false;

        assert!(carve_block(
            &settings,
            &mut chunk,
            &preliminary_surfaces,
            8,
            8,
            62,
            8,
            8,
            settings.min_y + 8,
            &mut has_grass,
        ));

        assert!(chunk.layer(8, 62, 8, settings.min_y).unwrap().is("minecraft:water"));
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
            feature.feature_index == 2
                && matches!(feature.count, OrePlacementCount::Constant(46))
                && feature
                    .ore
                    .targets
                    .iter()
                    .any(|target| {
                        matches!(target.predicate, OreTargetPredicate::BaseStoneOverworld)
                            && target.block.block.as_ref() == "minecraft:clay"
                    })
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == LUSH_CAVES_ORE_BIOMES)
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
        assert!(disks.iter().any(|feature| {
            feature.feature_index == 30
                && feature.target_blocks == DISK_DIRT_MUD_TARGETS
                && matches!(
                    feature.surface_anchor,
                    Some(SurfaceDiskAnchor {
                        block: "minecraft:mud",
                        offset_y: -1
                    })
                )
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == MANGROVE_TREE_BIOMES)
        }));
    }

    #[test]
    fn vanilla_noise_configures_surface_decorations() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.surface_features;

        let forest_rock = features
            .iter()
            .find(|feature| feature.feature_index == 1 && feature.step_index == 2)
            .unwrap();
        assert!(matches!(forest_rock.count, OrePlacementCount::Constant(2)));
        assert!(matches!(
            forest_rock.config,
            SurfaceFeatureConfig::BlockBlob(_)
        ));
        assert!(matches!(
            forest_rock.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == FOREST_ROCK_BIOMES
        ));

        let ice_spike = features
            .iter()
            .find(|feature| feature.feature_index == 0 && feature.step_index == 4)
            .unwrap();
        assert!(matches!(ice_spike.count, OrePlacementCount::Constant(3)));
        assert!(matches!(ice_spike.config, SurfaceFeatureConfig::IceSpike(_)));

        let iceberg_packed = features
            .iter()
            .find(|feature| feature.feature_index == 2 && feature.step_index == 2)
            .unwrap();
        assert!(matches!(iceberg_packed.count, OrePlacementCount::Rarity(16)));
        assert!(matches!(
            iceberg_packed.heightmap,
            SurfaceHeightmap::SeaLevel
        ));
        assert!(matches!(
            iceberg_packed.config,
            SurfaceFeatureConfig::Iceberg(_)
        ));
        assert!(matches!(
            iceberg_packed.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == ICEBERG_BIOMES
        ));

        let iceberg_blue = features
            .iter()
            .find(|feature| feature.feature_index == 3 && feature.step_index == 2)
            .unwrap();
        assert!(matches!(iceberg_blue.count, OrePlacementCount::Rarity(200)));
        assert!(matches!(
            iceberg_blue.config,
            SurfaceFeatureConfig::Iceberg(_)
        ));

        let ice_patch = features
            .iter()
            .find(|feature| feature.feature_index == 1 && feature.step_index == 4)
            .unwrap();
        assert_eq!(ice_patch.y_offset, -1);
        assert!(matches!(ice_patch.config, SurfaceFeatureConfig::Disk(_)));

        let blue_ice = features
            .iter()
            .find(|feature| feature.feature_index == 4 && feature.step_index == 4)
            .unwrap();
        assert!(matches!(
            blue_ice.count,
            OrePlacementCount::Uniform { min: 0, max: 19 }
        ));
        assert!(matches!(
            blue_ice.heightmap,
            SurfaceHeightmap::HeightRange(OreHeight::Uniform(
                HeightAnchor::Absolute(30),
                HeightAnchor::Absolute(61)
            ))
        ));
        assert!(matches!(blue_ice.config, SurfaceFeatureConfig::BlueIce(_)));

        let pale_moss = features
            .iter()
            .find(|feature| feature.feature_index == 90)
            .unwrap();
        assert_eq!(pale_moss.step_index, 9);
        assert!(matches!(
            pale_moss.config,
            SurfaceFeatureConfig::VegetationPatch(_)
        ));
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
        assert!(springs.iter().any(|feature| {
            feature.step_index == 8
                && feature.feature_index == 2
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
                && feature.config.valid_blocks == SPRING_FROZEN_LAVA_VALID_BLOCKS
                && matches!(
                    feature.biome_filter,
                    FeatureBiomeFilter::Include(biomes) if biomes == FROZEN_LAVA_SPRING_BIOMES
                )
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
    fn vanilla_noise_configures_dripstone_cave_features() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.dripstone_features;

        let large = features
            .iter()
            .find_map(|feature| match feature {
                PlacedDripstoneFeature::Large(feature) => Some(feature),
                _ => None,
            })
            .unwrap();
        assert_eq!(large.step_index, 2);
        assert_eq!(large.feature_index, 1);
        assert!(matches!(
            large.count,
            OrePlacementCount::Uniform { min: 10, max: 48 }
        ));
        assert!(matches!(
            large.height,
            OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256))
        ));
        assert_eq!(large.config.column_radius.min, 3);
        assert_eq!(large.config.column_radius.max, 19);
        assert_eq!(large.config.floor_to_ceiling_search_range, 30);
        assert!(matches!(
            large.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DRIPSTONE_CAVES_BIOMES
        ));

        let cluster = features
            .iter()
            .find_map(|feature| match feature {
                PlacedDripstoneFeature::Cluster(feature) => Some(feature),
                _ => None,
            })
            .unwrap();
        assert_eq!(cluster.step_index, 7);
        assert_eq!(cluster.feature_index, 0);
        assert!(matches!(
            cluster.count,
            OrePlacementCount::Uniform { min: 48, max: 96 }
        ));
        assert_eq!(cluster.config.radius.min, 2);
        assert_eq!(cluster.config.radius.max, 8);
        assert_eq!(cluster.config.floor_to_ceiling_search_range, 12);
        assert!(matches!(
            cluster.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DRIPSTONE_CAVES_BIOMES
        ));

        let pointed = features
            .iter()
            .find_map(|feature| match feature {
                PlacedDripstoneFeature::Pointed(feature) => Some(feature),
                _ => None,
            })
            .unwrap();
        assert_eq!(pointed.step_index, 7);
        assert_eq!(pointed.feature_index, 1);
        assert!(matches!(
            pointed.outer_count,
            OrePlacementCount::Uniform { min: 192, max: 256 }
        ));
        assert!(matches!(
            pointed.inner_count,
            OrePlacementCount::Uniform { min: 1, max: 5 }
        ));
        assert_eq!(pointed.xz_offset.min, -10);
        assert_eq!(pointed.xz_offset.max, 10);
        assert_eq!(pointed.y_offset.min, -2);
        assert_eq!(pointed.y_offset.max, 2);
        assert!(matches!(
            pointed.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DRIPSTONE_CAVES_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_deep_dark_sculk_features() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.sculk_features;

        let vein = features
            .iter()
            .find_map(|feature| match feature {
                PlacedSculkFeature::Vein(feature) => Some(feature),
                _ => None,
            })
            .unwrap();
        assert_eq!(vein.step_index, 7);
        assert_eq!(vein.feature_index, 0);
        assert!(matches!(
            vein.count,
            OrePlacementCount::Uniform { min: 204, max: 250 }
        ));
        assert!(matches!(
            vein.height,
            OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256))
        ));
        assert_eq!(vein.config.block.block.as_ref(), "minecraft:sculk_vein");
        assert!(vein.config.can_place_on_floor);
        assert!(vein.config.can_place_on_ceiling);
        assert!(vein.config.can_place_on_wall);
        assert_eq!(vein.config.chance_of_spreading, 1.0);
        assert!(matches!(
            vein.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DEEP_DARK_BIOMES
        ));

        let patch = features
            .iter()
            .find_map(|feature| match feature {
                PlacedSculkFeature::Patch(feature) => Some(feature),
                _ => None,
            })
            .unwrap();
        assert_eq!(patch.step_index, 7);
        assert_eq!(patch.feature_index, 1);
        assert!(matches!(patch.count, OrePlacementCount::Constant(256)));
        assert_eq!(patch.config.charge_count, 10);
        assert_eq!(patch.config.amount_per_charge, 32);
        assert_eq!(patch.config.spread_attempts, 64);
        assert_eq!(patch.config.spread_rounds, 1);
        assert_eq!(patch.config.growth_rounds, 0);
        assert_eq!(patch.config.catalyst_chance, 0.5);
        assert!(matches!(
            patch.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DEEP_DARK_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_structure_features() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.structure_features;

        let desert_well = features
            .iter()
            .find_map(|feature| match feature {
                PlacedStructureFeature::DesertWell(feature) => Some(feature),
                _ => None,
            })
            .unwrap();
        assert_eq!(desert_well.step_index, 4);
        assert_eq!(desert_well.feature_index, 0);
        assert_eq!(desert_well.rarity, 1000);
        assert!(matches!(
            desert_well.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == DESERT_WELL_BIOMES
        ));

        let fossils: Vec<_> = features
            .iter()
            .filter_map(|feature| match feature {
                PlacedStructureFeature::Fossil(feature) => Some(feature),
                _ => None,
            })
            .collect();
        assert_eq!(fossils.len(), 2);
        assert!(fossils.iter().any(|feature| {
            feature.step_index == 3
                && feature.feature_index == 2
                && feature.rarity == 64
                && feature.config.overlay_block.is("minecraft:coal_ore")
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::Absolute(0), HeightAnchor::BelowTop(0))
                )
                && matches!(
                    feature.biome_filter,
                    FeatureBiomeFilter::Include(biomes) if biomes == FOSSIL_BIOMES
                )
        }));
        assert!(fossils.iter().any(|feature| {
            feature.step_index == 3
                && feature.feature_index == 3
                && feature.rarity == 64
                && feature.config.overlay_block.is("minecraft:diamond_ore")
                && matches!(
                    feature.height,
                    OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(-8))
                )
                && matches!(
                    feature.biome_filter,
                    FeatureBiomeFilter::Include(biomes) if biomes == FOSSIL_BIOMES
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
    fn vanilla_noise_configures_lush_cave_decorations() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let cave_vines = &settings.ore_features.cave_vines;
        let spore_blossom = &settings.ore_features.spore_blossom;
        let classic_vines = &settings.ore_features.classic_vines;

        assert_eq!(cave_vines.step_index, 9);
        assert_eq!(cave_vines.feature_index, 77);
        assert!(matches!(
            cave_vines.count,
            OrePlacementCount::Constant(188)
        ));
        assert_eq!(cave_vines.search_range, 12);
        assert_eq!(cave_vines.random_y_offset, -1);
        assert!(matches!(
            cave_vines.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == LUSH_CAVES_BIOMES
        ));

        assert_eq!(spore_blossom.step_index, 9);
        assert_eq!(spore_blossom.feature_index, 78);
        assert!(matches!(
            spore_blossom.count,
            OrePlacementCount::Constant(25)
        ));
        assert_eq!(spore_blossom.search_range, 12);
        assert_eq!(spore_blossom.random_y_offset, -1);
        assert!(matches!(
            spore_blossom.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == LUSH_CAVES_BIOMES
        ));

        assert_eq!(classic_vines.step_index, 9);
        assert_eq!(classic_vines.feature_index, 83);
        assert!(matches!(
            classic_vines.count,
            OrePlacementCount::Constant(256)
        ));
        assert!(matches!(
            classic_vines.height,
            OreHeight::Uniform(HeightAnchor::AboveBottom(0), HeightAnchor::Absolute(256))
        ));
        assert!(matches!(
            classic_vines.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == LUSH_CAVES_BIOMES
        ));

        let surface_vines = &settings.ore_features.surface_vines;
        assert_eq!(surface_vines.step_index, 9);
        assert_eq!(surface_vines.feature_index, 85);
        assert!(matches!(
            surface_vines.count,
            OrePlacementCount::Constant(127)
        ));
        assert!(matches!(
            surface_vines.height,
            OreHeight::Uniform(HeightAnchor::Absolute(64), HeightAnchor::Absolute(100))
        ));
        assert!(matches!(
            surface_vines.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == SURFACE_VINES_BIOMES
        ));
    }

    #[test]
    fn vanilla_noise_configures_lush_environment_scan_features() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.environment_scan_features;

        assert!(features.iter().any(|feature| {
            feature.feature_index == 79
                && matches!(feature.count, OrePlacementCount::Constant(125))
                && matches!(feature.search, EnvironmentScan::Up { max_steps: 12 })
                && feature.random_y_offset == -1
                && matches!(feature.config, EnvironmentFeatureConfig::VegetationPatch(_))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 80
                && matches!(feature.count, OrePlacementCount::Constant(62))
                && matches!(feature.search, EnvironmentScan::Down { max_steps: 12 })
                && feature.random_y_offset == 1
                && matches!(feature.config, EnvironmentFeatureConfig::VegetationPatch(_))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 81
                && matches!(feature.count, OrePlacementCount::Constant(125))
                && matches!(feature.search, EnvironmentScan::Down { max_steps: 12 })
                && feature.random_y_offset == 1
                && matches!(feature.config, EnvironmentFeatureConfig::VegetationPatch(_))
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 82
                && matches!(feature.count, OrePlacementCount::Uniform { min: 1, max: 2 })
                && matches!(feature.search, EnvironmentScan::Up { max_steps: 12 })
                && feature.random_y_offset == -1
                && matches!(feature.config, EnvironmentFeatureConfig::RootedAzaleaTree(_))
        }));
    }

    #[test]
    fn vanilla_noise_configures_aquatic_features() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let features = &settings.ore_features.aquatic_features;

        assert!(features.iter().any(|feature| {
            feature.step_index == 9
                && feature.feature_index == 66
                && matches!(feature.placement, AquaticPlacement::Count(48))
                && matches!(feature.config, AquaticFeatureConfig::Seagrass { tall_probability, .. } if tall_probability == 0.3)
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == SEAGRASS_NORMAL_BIOMES)
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 70
                && matches!(feature.placement, AquaticPlacement::Count(48))
                && matches!(feature.config, AquaticFeatureConfig::Seagrass { tall_probability, .. } if tall_probability == 0.8)
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == SEAGRASS_DEEP_WARM_BIOMES)
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 74
                && matches!(
                    feature.placement,
                    AquaticPlacement::NoiseBasedCount {
                        noise_to_count_ratio: 120,
                        ..
                    }
                )
                && matches!(feature.config, AquaticFeatureConfig::Kelp { .. })
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == KELP_COLD_BIOMES)
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 76
                && matches!(feature.placement, AquaticPlacement::Rarity { chance: 16 })
                && matches!(feature.config, AquaticFeatureConfig::SeaPickle { count: 20, .. })
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == SEA_PICKLE_BIOMES)
        }));
        assert!(features.iter().any(|feature| {
            feature.feature_index == 102
                && matches!(
                    feature.placement,
                    AquaticPlacement::NoiseBasedCount {
                        noise_to_count_ratio: 20,
                        noise_factor: 400.0,
                        noise_offset: 0.0
                    }
                )
                && matches!(feature.config, AquaticFeatureConfig::Coral(_))
                && matches!(feature.biome_filter, FeatureBiomeFilter::Include(biomes) if biomes == WARM_OCEAN_VEGETATION_BIOMES)
        }));
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
    fn vanilla_noise_configures_patch_tall_grass() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let feature = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 0)
            .unwrap();

        assert_eq!(feature.step_index, 9);
        assert_eq!(feature.feature_index, 0);
        assert!(feature.noise_threshold.is_none());
        assert_eq!(feature.rarity, 5);
        assert_eq!(feature.inner_count, 96);
        assert!(feature.block.lower.is("minecraft:tall_grass"));
        assert!(feature.block.upper.is_some());
        assert!(matches!(
            feature.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == PATCH_TALL_GRASS_BIOMES
        ));
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
        let bamboo_light = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 99)
            .unwrap();
        let bamboo_some_podzol = settings
            .ore_features
            .block_columns
            .iter()
            .find(|feature| feature.feature_index == 100)
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

        assert_eq!(bamboo_light.rarity, 4);
        assert!(bamboo_light.column.block.is("minecraft:bamboo"));
        assert!(matches!(
            bamboo_light.column.kind,
            BlockColumnKind::Bamboo {
                podzol_probability
            } if podzol_probability == 0.0
        ));
        assert!(matches!(
            bamboo_light.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == BAMBOO_LIGHT_BIOMES
        ));

        assert_eq!(bamboo_some_podzol.rarity, 1);
        assert!(matches!(
            bamboo_some_podzol.outer_count,
            BlockColumnOuterCount::NoiseBased {
                noise_to_count_ratio: 160,
                noise_factor,
                noise_offset
            } if noise_factor == 80.0 && noise_offset == 0.3
        ));
        assert!(matches!(
            bamboo_some_podzol.column.kind,
            BlockColumnKind::Bamboo {
                podzol_probability
            } if podzol_probability == 0.2
        ));
        assert!(matches!(
            bamboo_some_podzol.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == BAMBOO_SOME_PODZOL_BIOMES
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
        let mushroom_island = settings
            .ore_features
            .huge_mushrooms
            .iter()
            .find(|feature| feature.feature_index == 101)
            .unwrap();
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

        assert_eq!(mushroom_island.step_index, 9);
        assert!(matches!(
            mushroom_island.count,
            OrePlacementCount::Constant(1)
        ));
        assert!(matches!(
            mushroom_island.config.selector,
            HugeMushroomSelector::RandomBoolean
        ));
        assert!(matches!(
            mushroom_island.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == MUSHROOM_ISLAND_VEGETATION_BIOMES
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

        let waterlily = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 84)
            .unwrap();
        assert_eq!(waterlily.outer_count, 4);
        assert_eq!(waterlily.rarity, 1);
        assert_eq!(waterlily.inner_count, 10);
        assert!(waterlily.block.lower.is("minecraft:lily_pad"));
        assert!(matches!(
            waterlily.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == WATERLILY_BIOMES
        ));

        let berry = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 85)
            .unwrap();
        assert_eq!(berry.rarity, 32);
        assert_eq!(berry.inner_count, 96);
        assert!(berry.block.lower.is("minecraft:sweet_berry_bush"));
        assert!(berry
            .block
            .lower
            .properties
            .iter()
            .any(|(name, value)| name == "age" && value == "3"));
        assert_eq!(berry.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            berry.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == BERRY_COMMON_BIOMES
        ));

        let rare_berry = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 86)
            .unwrap();
        assert_eq!(rare_berry.rarity, 384);
        assert!(matches!(
            rare_berry.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == BERRY_RARE_BIOMES
        ));

        let firefly_swamp = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 87)
            .unwrap();
        assert_eq!(firefly_swamp.rarity, 8);
        assert_eq!(firefly_swamp.inner_count, 20);
        assert!(firefly_swamp.block.lower.is("minecraft:firefly_bush"));

        let firefly_near_water = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 88)
            .unwrap();
        assert_eq!(firefly_near_water.outer_count, 2);
        assert!(matches!(
            firefly_near_water.placement_predicate,
            SimpleVegetationPlacementPredicate::AirSurvivesNearWater
        ));

        let meadow = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 91)
            .unwrap();
        assert_eq!(meadow.inner_count, 96);
        assert!(matches!(
            &meadow.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.iter().any(|(layer, _)| layer.is("minecraft:allium"))
                    && entries.iter().any(|(layer, _)| layer.is("minecraft:short_grass"))
        ));
        assert!(matches!(
            meadow.biome_filter,
            FeatureBiomeFilter::Include(biomes) if biomes == FLOWER_MEADOW_BIOMES
        ));

        let flower_forest = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 92)
            .unwrap();
        assert_eq!(flower_forest.outer_count, 3);
        assert_eq!(flower_forest.rarity, 2);
        assert!(matches!(
            &flower_forest.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.iter().any(|(layer, _)| layer.is("minecraft:lily_of_the_valley"))
                    && entries.iter().any(|(layer, _)| layer.is("minecraft:red_tulip"))
        ));

        let forest_flowers = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 93)
            .unwrap();
        assert!(matches!(
            forest_flowers.count_provider,
            SimpleVegetationCountProvider::ClampedUniform {
                min: -3,
                max: 1,
                clamp_min: 0,
                clamp_max: 1
            }
        ));
        assert!(matches!(
            &forest_flowers.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.iter().any(|(layer, _)| layer.is("minecraft:lilac"))
                    && entries.iter().any(|(layer, _)| layer.is("minecraft:rose_bush"))
                    && entries.iter().any(|(layer, _)| layer.is("minecraft:peony"))
        ));

        let leaf_litter = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 95)
            .unwrap();
        assert_eq!(leaf_litter.outer_count, 2);
        assert_eq!(leaf_litter.inner_count, 32);
        assert_eq!(leaf_litter.required_support, Some("minecraft:grass_block"));
        assert!(matches!(
            &leaf_litter.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.len() == 12
                    && entries.iter().all(|(layer, _)| layer.is("minecraft:leaf_litter"))
        ));

        let wildflowers = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 96)
            .unwrap();
        assert_eq!(wildflowers.inner_count, 8);
        assert!(wildflowers.noise_threshold.is_some());
        assert!(matches!(
            &wildflowers.block.provider,
            SimpleVegetationProvider::Weighted { entries }
                if entries.len() == 16
                    && entries.iter().all(|(layer, _)| layer.is("minecraft:wildflowers"))
        ));

        let pale_garden_flowers = settings
            .ore_features
            .vegetation_patches
            .iter()
            .find(|feature| feature.feature_index == 98)
            .unwrap();
        assert_eq!(pale_garden_flowers.rarity, 8);
        assert!(pale_garden_flowers
            .block
            .lower
            .is("minecraft:closed_eyeblossom"));
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
        assert_eq!(dark_forest.config.mushroom_variants.len(), 2);
        assert_eq!(
            dark_forest.config.mushroom_variants[0].chance,
            0.025
        );
        assert!(matches!(
            dark_forest.config.mushroom_variants[0].config.selector,
            HugeMushroomSelector::Single(HugeMushroomKind::Brown)
        ));
        assert_eq!(
            dark_forest.config.mushroom_variants[1].chance,
            0.05
        );
        assert!(matches!(
            dark_forest.config.mushroom_variants[1].config.selector,
            HugeMushroomSelector::Single(HugeMushroomKind::Red)
        ));
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

    fn cave_floor_test_chunk(settings: &NoiseSettings, floor_y: i32) -> NoiseChunkBlocks {
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= floor_y {
                            stone.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: floor_y + 1 - settings.min_y,
            })
            .collect();

        NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        }
    }

    fn cave_room_test_chunk(settings: &NoiseSettings, floor_y: i32, ceiling_y: i32) -> NoiseChunkBlocks {
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= floor_y || y >= ceiling_y {
                            stone.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: settings.height,
            })
            .collect();

        NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        }
    }

    fn water_surface_test_chunk(
        settings: &NoiseSettings,
        floor_y: i32,
        water_top_y: i32,
    ) -> NoiseChunkBlocks {
        let dirt = BlockLayer::new("minecraft:dirt");
        let water = BlockLayer::new("minecraft:water");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|_| NoiseColumnBlocks {
                blocks: (settings.min_y..settings.min_y + settings.height)
                    .map(|y| {
                        if y <= floor_y {
                            dirt.clone()
                        } else if y <= water_top_y {
                            water.clone()
                        } else {
                            air.clone()
                        }
                    })
                    .collect(),
                first_available_height: water_top_y + 1 - settings.min_y,
            })
            .collect();

        NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        }
    }

    #[test]
    fn vanilla_noise_cave_vines_hang_from_ceiling_with_tip() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_ceiling_test_chunk(&settings, 70);
        let mut random = FeatureRandom::new(7);

        assert!(CaveVinesFeatureConfig::new().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut random,
            8,
            69,
            8,
        ));

        let placed = (settings.min_y..70)
            .filter_map(|y| chunk.layer(8, y, 8, settings.min_y).map(|layer| (y, layer)))
            .filter(|(_, layer)| {
                layer.is("minecraft:cave_vines") || layer.is("minecraft:cave_vines_plant")
            })
            .collect::<Vec<_>>();
        assert!(!placed.is_empty());
        assert!(placed.iter().any(|(_, layer)| layer.is("minecraft:cave_vines")));
        assert!(placed.iter().all(|(_, layer)| {
            layer
                .properties
                .iter()
                .any(|(name, value)| name == "berries" && matches!(value.as_str(), "true" | "false"))
        }));
    }

    #[test]
    fn vanilla_noise_spore_blossom_places_under_solid_ceiling() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_ceiling_test_chunk(&settings, 70);
        let mut feature = PlacedSporeBlossomFeature::new(0);
        feature.biome_filter = FeatureBiomeFilter::All;
        feature.height = OreHeight::Uniform(HeightAnchor::Absolute(58), HeightAnchor::Absolute(69));
        let mut random = FeatureRandom::new(10);

        feature.place(&settings, 0, 0, &mut chunk, &mut random);

        let has_spore_blossom = chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| layer.is("minecraft:spore_blossom"))
        });
        assert!(has_spore_blossom);
    }

    #[test]
    fn vanilla_noise_pointed_dripstone_grows_from_floor_and_ceiling() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let config = PointedDripstoneFeatureConfig::new();

        let mut floor_chunk = cave_floor_test_chunk(&settings, 63);
        assert!(config.place_at(
            &settings,
            0,
            0,
            &mut floor_chunk,
            &mut FeatureRandom::new(3),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_property(
            &floor_chunk,
            "minecraft:pointed_dripstone",
            "vertical_direction",
            "up"
        ));

        let mut ceiling_chunk = cave_ceiling_test_chunk(&settings, 70);
        assert!(config.place_at(
            &settings,
            0,
            0,
            &mut ceiling_chunk,
            &mut FeatureRandom::new(3),
            8,
            69,
            8,
        ));
        assert!(chunk_contains_property(
            &ceiling_chunk,
            "minecraft:pointed_dripstone",
            "vertical_direction",
            "down"
        ));
    }

    #[test]
    fn vanilla_noise_pointed_dripstone_placed_feature_scans_cave_space() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_room_test_chunk(&settings, 63, 72);
        let mut feature = PlacedPointedDripstoneFeature::new(0);
        feature.biome_filter = FeatureBiomeFilter::All;
        feature.outer_count = OrePlacementCount::Constant(1);
        feature.inner_count = OrePlacementCount::Constant(1);
        feature.height = OreHeight::Uniform(HeightAnchor::Absolute(67), HeightAnchor::Absolute(67));
        feature.xz_offset = ClampedNormalInt {
            mean: 0.0,
            deviation: 0.0,
            min: 0,
            max: 0,
        };
        feature.y_offset = ClampedNormalInt {
            mean: 0.0,
            deviation: 0.0,
            min: 0,
            max: 0,
        };

        feature.place(&settings, 0, 0, &mut chunk, &mut FeatureRandom::new(5));

        assert!(chunk_contains_block(&chunk, "minecraft:pointed_dripstone"));
    }

    #[test]
    fn vanilla_noise_dripstone_cluster_places_blocks_and_points() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let config = DripstoneClusterFeatureConfig::new();
        let mut placed = None;

        for seed in 0..64 {
            let mut chunk = cave_room_test_chunk(&settings, 63, 74);
            if config.place(
                &settings,
                0,
                0,
                &mut chunk,
                &mut FeatureRandom::new(seed),
                8,
                68,
                8,
            ) && chunk_contains_block(&chunk, "minecraft:dripstone_block")
                && chunk_contains_block(&chunk, "minecraft:pointed_dripstone")
            {
                placed = Some(chunk);
                break;
            }
        }

        let chunk = placed.expect("test seeds should place a dripstone cluster");
        assert!(chunk_contains_property(
            &chunk,
            "minecraft:pointed_dripstone",
            "thickness",
            "tip"
        ) || chunk_contains_property(
            &chunk,
            "minecraft:pointed_dripstone",
            "thickness",
            "tip_merge"
        ));
    }

    #[test]
    fn vanilla_noise_large_dripstone_places_cone() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_room_test_chunk(&settings, 48, 88);
        let config = LargeDripstoneFeatureConfig::new();

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(7),
            8,
            68,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:dripstone_block"));
    }

    #[test]
    fn vanilla_noise_sculk_vein_attaches_to_cave_surface() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_room_test_chunk(&settings, 63, 72);

        assert!(place_sculk_vein(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(2),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:sculk_vein"));
        assert!(
            chunk_contains_property(&chunk, "minecraft:sculk_vein", "up", "true")
                || chunk_contains_property(&chunk, "minecraft:sculk_vein", "down", "true")
                || chunk_contains_property(&chunk, "minecraft:sculk_vein", "north", "true")
                || chunk_contains_property(&chunk, "minecraft:sculk_vein", "south", "true")
                || chunk_contains_property(&chunk, "minecraft:sculk_vein", "west", "true")
                || chunk_contains_property(&chunk, "minecraft:sculk_vein", "east", "true")
        );
    }

    #[test]
    fn vanilla_noise_sculk_patch_spreads_over_cave_surface() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let config = SculkPatchFeatureConfig::deep_dark();
        let mut chunk = cave_room_test_chunk(&settings, 63, 72);

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(4),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:sculk"));
        assert!(
            chunk_contains_block(&chunk, "minecraft:sculk_vein")
                || chunk_contains_block(&chunk, "minecraft:sculk_catalyst")
        );
    }

    #[test]
    fn vanilla_noise_desert_well_places_water_and_suspicious_sand() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let config = DesertWellFeatureConfig::new();
        let mut chunk = surface_test_chunk(&settings, 63, "minecraft:sand");

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(8),
            8,
            64,
            8,
        ));

        assert!(chunk_contains_block(&chunk, "minecraft:sandstone"));
        assert!(chunk_contains_block(&chunk, "minecraft:water"));
        assert!(chunk_contains_block(&chunk, "minecraft:suspicious_sand"));
        assert!(chunk.block_entities.iter().any(|entity| {
            entity.entity_type == BRUSHABLE_BLOCK_ENTITY_TYPE_ID
                && matches!(
                    &entity.nbt,
                    Tag::Compound(fields)
                        if matches!(
                            fields.get("LootTable"),
                            Some(Tag::String(name)) if name.as_ref() == "minecraft:archaeology/desert_well"
                        )
                        && matches!(fields.get("LootTableSeed"), Some(Tag::Long(_)))
                )
        }));
    }

    #[test]
    fn vanilla_noise_fossil_places_bone_and_overlay_ores() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut coal_chunk = surface_test_chunk(&settings, 80, "minecraft:stone");
        let mut diamond_chunk = surface_test_chunk(&settings, 0, "minecraft:deepslate");

        assert!(FossilFeatureConfig::new("minecraft:coal_ore").place(
            &settings,
            0,
            0,
            &mut coal_chunk,
            &mut FeatureRandom::new(2),
            8,
            40,
            8,
        ));
        assert!(FossilFeatureConfig::new("minecraft:diamond_ore").place(
            &settings,
            0,
            0,
            &mut diamond_chunk,
            &mut FeatureRandom::new(2),
            8,
            -32,
            8,
        ));

        assert!(chunk_contains_block(&coal_chunk, "minecraft:bone_block"));
        assert!(chunk_contains_block(&coal_chunk, "minecraft:coal_ore"));
        assert!(chunk_contains_block(&diamond_chunk, "minecraft:bone_block"));
        assert!(chunk_contains_block(&diamond_chunk, "minecraft:diamond_ore"));
    }

    #[test]
    fn vanilla_noise_classic_vines_attach_to_cave_wall() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let stone = BlockLayer::new("minecraft:stone");
        let air = BlockLayer::new("minecraft:air");
        let columns = (0..HEIGHTMAP_ENTRY_COUNT)
            .map(|index| {
                let x = index % 16;
                NoiseColumnBlocks {
                    blocks: (settings.min_y..settings.min_y + settings.height)
                        .map(|_| if x == 7 { stone.clone() } else { air.clone() })
                        .collect(),
                    first_available_height: settings.height,
                }
            })
            .collect();
        let mut chunk = NoiseChunkBlocks {
            columns,
            biomes: Vec::new(),
            block_entities: Vec::new(),
        };
        let feature = PlacedClassicVinesFeature::cave(0);

        assert!(feature.place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(2),
            8,
            64,
            8,
        ));
        let vine = chunk.layer(8, 64, 8, settings.min_y).unwrap();
        assert!(vine.is("minecraft:vine"));
        assert!(vine.properties.iter().any(|(name, value)| {
            matches!(name.as_str(), "north" | "south" | "west" | "east") && value == "true"
        }));
    }

    #[test]
    fn vanilla_noise_moss_patch_replaces_floor_and_places_lush_vegetation() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_floor_test_chunk(&settings, 63);
        let config = VegetationPatchConfig::moss_patch();

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(12),
            8,
            63,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:moss_block"));
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                matches!(
                    layer.block.as_ref(),
                    "minecraft:azalea"
                        | "minecraft:flowering_azalea"
                        | "minecraft:moss_carpet"
                        | "minecraft:short_grass"
                        | "minecraft:tall_grass"
                )
            })
        }));
    }

    #[test]
    fn vanilla_noise_lush_clay_patch_can_place_dripleaf() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = underwater_test_chunk(&settings, 63, 67, "minecraft:stone");
        let config = VegetationPatchConfig::lush_caves_clay();

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(7),
            8,
            63,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:clay"));
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                matches!(
                    layer.block.as_ref(),
                    "minecraft:small_dripleaf"
                        | "minecraft:big_dripleaf"
                        | "minecraft:big_dripleaf_stem"
                )
            })
        }));
    }

    #[test]
    fn vanilla_noise_rooted_azalea_tree_places_roots_and_azalea_leaves() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);
        let config = RootedAzaleaTreeConfig::new();

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(3),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:rooted_dirt"));
        assert!(chunk_contains_block(&chunk, "minecraft:azalea_leaves"));
    }

    #[test]
    fn vanilla_noise_waterlily_places_on_water_surface() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = water_surface_test_chunk(&settings, 61, 63);

        assert!(SimpleVegetationBlock::waterlily().place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(1),
            8,
            64,
            8,
        ));
        assert!(chunk.layer(8, 64, 8, settings.min_y).unwrap().is("minecraft:lily_pad"));
        assert!(chunk.layer(8, 63, 8, settings.min_y).unwrap().is("minecraft:water"));
    }

    #[test]
    fn vanilla_noise_surface_blob_places_forest_rock_on_substrate() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);
        let config = BlockBlobSurfaceConfig::forest_rock();

        assert!(config.place_resolved(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(4),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:mossy_cobblestone"));
    }

    #[test]
    fn vanilla_noise_ice_spike_requires_snow_and_places_packed_ice() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_layer(x, 63, z, settings.min_y, BlockLayer::new("minecraft:snow_block"));
            }
        }
        let config = IceSpikeSurfaceConfig::new();

        assert!(config.find_origin_y(&settings, 0, 0, &chunk, 8, 64, 8).is_some());
        assert!(config.place_resolved(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(7),
            8,
            63,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:packed_ice"));
    }

    #[test]
    fn vanilla_noise_ice_patch_replaces_snowy_surface() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_layer(x, 63, z, settings.min_y, BlockLayer::new("minecraft:snow_block"));
            }
        }
        let config = SurfaceDiskConfig::ice_patch();

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(2),
            8,
            63,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:packed_ice"));
    }

    #[test]
    fn vanilla_noise_iceberg_places_large_ice_body() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = underwater_test_chunk(&settings, 45, settings.sea_level, "minecraft:sand");

        assert!(IcebergSurfaceConfig::packed().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(1),
            8,
            settings.sea_level,
            8,
        ));

        assert!(chunk_contains_block(&chunk, "minecraft:packed_ice"));
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().enumerate().any(|(index, layer)| {
                settings.min_y + index as i32 > settings.sea_level
                    && layer.is("minecraft:packed_ice")
            })
        }));
    }

    #[test]
    fn vanilla_noise_blue_ice_spreads_from_packed_ice_neighbor() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = underwater_test_chunk(&settings, 45, settings.sea_level, "minecraft:sand");
        chunk.set_layer(
            7,
            55,
            8,
            settings.min_y,
            BlockLayer::new("minecraft:packed_ice"),
        );

        assert!(BlueIceSurfaceConfig::new().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(2),
            8,
            55,
            8,
        ));

        assert!(chunk_contains_block(&chunk, "minecraft:blue_ice"));
        assert!(
            chunk
                .layer(8, 55, 8, settings.min_y)
                .is_some_and(|layer| layer.is("minecraft:blue_ice"))
        );
    }

    #[test]
    fn vanilla_noise_pale_moss_patch_places_pale_moss_vegetation() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = cave_floor_test_chunk(&settings, 63);
        let config = VegetationPatchConfig::pale_moss_patch();

        assert!(config.place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(9),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:pale_moss_block"));
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                matches!(
                    layer.block.as_ref(),
                    "minecraft:pale_moss_carpet" | "minecraft:short_grass" | "minecraft:tall_grass"
                )
            })
        }));
    }

    #[test]
    fn vanilla_noise_firefly_bush_near_water_requires_adjacent_water() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);
        let water = BlockLayer::new("minecraft:water");

        assert!(!SimpleVegetationPlacementPredicate::AirSurvivesNearWater.allows(
            &chunk,
            0,
            0,
            8,
            64,
            8,
            settings.min_y,
        ));

        chunk.set_layer(9, 63, 8, settings.min_y, water);
        assert!(SimpleVegetationPlacementPredicate::AirSurvivesNearWater.allows(
            &chunk,
            0,
            0,
            8,
            64,
            8,
            settings.min_y,
        ));
        assert!(SimpleVegetationBlock::single("minecraft:firefly_bush").place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(1),
            8,
            64,
            8,
        ));
    }

    #[test]
    fn vanilla_noise_disk_grass_turns_exposed_mud_to_grass() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_layer(x, 63, z, settings.min_y, BlockLayer::new("minecraft:mud"));
            }
        }
        let disk = PlacedDiskFeature::grass(30).with_surface_anchor("minecraft:mud", 0);

        disk.place_disk(&settings, 0, 0, &mut chunk, &mut FeatureRandom::new(1), 8, 63, 8);
        assert!(chunk_contains_block(&chunk, "minecraft:grass_block"));
    }

    #[test]
    fn vanilla_noise_weighted_ground_cover_places_leaf_litter_and_wildflowers() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut leaf_chunk = grass_surface_test_chunk(&settings, 63);
        let mut wildflower_chunk = grass_surface_test_chunk(&settings, 63);

        assert!(SimpleVegetationBlock::leaf_litter().place_at(
            &settings,
            0,
            0,
            &mut leaf_chunk,
            &mut FeatureRandom::new(1),
            8,
            64,
            8,
        ));
        assert!(leaf_chunk
            .layer(8, 64, 8, settings.min_y)
            .unwrap()
            .is("minecraft:leaf_litter"));

        assert!(SimpleVegetationBlock::wildflowers().place_at(
            &settings,
            0,
            0,
            &mut wildflower_chunk,
            &mut FeatureRandom::new(1),
            8,
            64,
            8,
        ));
        assert!(wildflower_chunk
            .layer(8, 64, 8, settings.min_y)
            .unwrap()
            .is("minecraft:wildflowers"));
    }

    #[test]
    fn vanilla_noise_bamboo_places_column_with_leaf_tip() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);

        assert!(BlockColumnFeatureConfig::bamboo(0.0).place_at(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(2),
            8,
            64,
            8,
        ));
        assert!(chunk_contains_block(&chunk, "minecraft:bamboo"));
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                layer.is("minecraft:bamboo")
                    && layer
                        .properties
                        .iter()
                        .any(|(name, value)| name == "leaves" && value == "large")
            })
        }));
    }

    #[test]
    fn vanilla_noise_huge_brown_mushroom_places_cap_and_stem() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);

        assert!(HugeMushroomFeatureConfig::brown().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(1),
            8,
            64,
            8,
        ));

        assert!(chunk_contains_block(&chunk, "minecraft:mushroom_stem"));
        assert!(chunk_contains_block(
            &chunk,
            "minecraft:brown_mushroom_block"
        ));
        assert!(
            chunk
                .layer(8, 64, 8, settings.min_y)
                .is_some_and(|layer| layer.is("minecraft:mushroom_stem"))
        );
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                layer.is("minecraft:brown_mushroom_block")
                    && layer
                        .properties
                        .iter()
                        .any(|(name, value)| name == "up" && value == "true")
            })
        }));
    }

    #[test]
    fn vanilla_noise_huge_red_mushroom_places_layered_cap() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = grass_surface_test_chunk(&settings, 63);

        assert!(HugeMushroomFeatureConfig::red().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(2),
            8,
            64,
            8,
        ));

        assert!(chunk_contains_block(&chunk, "minecraft:mushroom_stem"));
        assert!(chunk_contains_block(&chunk, "minecraft:red_mushroom_block"));
        assert!(chunk.columns.iter().any(|column| {
            column.blocks.iter().any(|layer| {
                layer.is("minecraft:red_mushroom_block")
                    && layer
                        .properties
                        .iter()
                        .any(|(name, value)| name == "up" && value == "false")
            })
        }));
    }

    #[test]
    fn vanilla_noise_huge_mushroom_spillover_places_neighbor_cap() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut source = grass_surface_test_chunk(&settings, 63);
        let mut target = grass_surface_test_chunk(&settings, 63);
        let mushroom = HugeMushroomFeatureConfig::brown();
        let mut random = FeatureRandom::new(12345);
        let mut replay_random = random.clone();

        assert!(mushroom.place(
            &settings,
            0,
            0,
            &mut source,
            &mut random,
            15,
            64,
            8,
        ));
        assert!(mushroom.place_spillover(
            &settings,
            16,
            0,
            &mut target,
            &mut replay_random,
            15,
            64,
            8,
        ));

        assert!(chunk_contains_block(
            &target,
            "minecraft:brown_mushroom_block"
        ));
        assert!(!chunk_contains_block(&target, "minecraft:mushroom_stem"));
    }

    #[test]
    fn vanilla_noise_seagrass_places_short_and_tall_variants_underwater() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut tall_chunk = underwater_test_chunk(&settings, 62, 70, "minecraft:sand");
        let mut short_chunk = tall_chunk.clone();

        assert!(AquaticFeatureConfig::seagrass(1.0).place(
            &settings,
            0,
            0,
            &mut tall_chunk,
            &mut FeatureRandom::new(1),
            8,
            63,
            8,
        ));
        assert_eq!(
            tall_chunk
                .layer(8, 63, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:tall_seagrass")
        );
        assert!(tall_chunk
            .layer(8, 63, 8, settings.min_y)
            .unwrap()
            .properties
            .iter()
            .any(|(name, value)| name == "half" && value == "lower"));
        assert!(tall_chunk
            .layer(8, 64, 8, settings.min_y)
            .unwrap()
            .properties
            .iter()
            .any(|(name, value)| name == "half" && value == "upper"));

        assert!(AquaticFeatureConfig::seagrass(0.0).place(
            &settings,
            0,
            0,
            &mut short_chunk,
            &mut FeatureRandom::new(1),
            8,
            63,
            8,
        ));
        assert_eq!(
            short_chunk
                .layer(8, 63, 8, settings.min_y)
                .map(|layer| layer.block.as_ref()),
            Some("minecraft:seagrass")
        );
    }

    #[test]
    fn vanilla_noise_kelp_places_column_with_aged_tip() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = underwater_test_chunk(&settings, 62, 70, "minecraft:gravel");

        assert!(AquaticFeatureConfig::kelp().place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(4),
            8,
            63,
            8,
        ));

        let kelp_blocks = (63..=70)
            .filter_map(|y| chunk.layer(8, y, 8, settings.min_y))
            .filter(|layer| layer.is("minecraft:kelp") || layer.is("minecraft:kelp_plant"))
            .collect::<Vec<_>>();
        assert!(!kelp_blocks.is_empty());
        assert!(kelp_blocks.last().unwrap().is("minecraft:kelp"));
        assert!(kelp_blocks.last().unwrap().properties.iter().any(|(name, value)| {
            name == "age" && matches!(value.as_str(), "20" | "21" | "22" | "23")
        }));
    }

    #[test]
    fn vanilla_noise_sea_pickle_places_waterlogged_cluster() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let mut chunk = underwater_test_chunk(&settings, 62, 70, "minecraft:sand");

        assert!(AquaticFeatureConfig::sea_pickle(20).place(
            &settings,
            0,
            0,
            &mut chunk,
            &mut FeatureRandom::new(8),
            8,
            63,
            8,
        ));

        let layer = chunk.layer(8, 63, 8, settings.min_y).unwrap();
        assert!(layer.is("minecraft:sea_pickle"));
        assert!(layer
            .properties
            .iter()
            .any(|(name, value)| name == "waterlogged" && value == "true"));
        assert!(layer.properties.iter().any(|(name, value)| {
            name == "pickles" && matches!(value.as_str(), "1" | "2" | "3" | "4")
        }));
    }

    #[test]
    fn vanilla_noise_warm_ocean_coral_places_reef_blocks() {
        let settings = NoiseSettings::overworld(12345, vanilla_noise::OverworldNoiseKind::Default);
        let config = CoralFeatureConfig::new();
        let mut placed = None;

        for seed in 0..16 {
            let mut chunk = underwater_test_chunk(&settings, 62, 82, "minecraft:stone");
            if config.place(
                &settings,
                0,
                0,
                &mut chunk,
                &mut FeatureRandom::new(seed),
                8,
                63,
                8,
            ) && chunk
                .columns
                .iter()
                .any(|column| column.blocks.iter().any(is_coral_layer))
            {
                placed = Some(chunk);
                break;
            }
        }

        let chunk = placed.expect("test seeds should place warm ocean coral");
        assert!(chunk.columns.iter().any(|column| {
            column
                .blocks
                .iter()
                .any(|layer| CORAL_BLOCKS.contains(&layer.block.as_ref()))
        }));
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
