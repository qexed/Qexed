#[derive(Debug, Clone)]
struct BlockLayer {
    block: Arc<str>,
    block_state_id: i32,
    properties: Arc<[(String, String)]>,
    is_air: bool,
}

impl BlockLayer {
    fn new(block: &str) -> Self {
        let block = normalize_identifier(block);
        let state = chunk_nbt::default_block_state(&block);
        Self {
            is_air: is_air_block(&block),
            block: Arc::from(block),
            block_state_id: state.id,
            properties: Arc::from(state.properties),
        }
    }

    fn with_properties(block: &str, properties: &[(&str, &str)]) -> Self {
        let block = normalize_identifier(block);
        let mut properties = properties
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect::<Vec<_>>();
        properties.sort_by(|left, right| left.0.cmp(&right.0));
        let state = chunk_nbt::block_state(&block, &properties);
        Self {
            is_air: is_air_block(&block),
            block: Arc::from(block),
            block_state_id: state.id,
            properties: Arc::from(state.properties),
        }
    }

    fn is(&self, block: &str) -> bool {
        self.block.as_ref() == block
    }

    fn with_property(&self, name: &str, value: &str) -> Self {
        let mut properties = self.properties.iter().cloned().collect::<Vec<_>>();
        if let Some((_, existing)) = properties
            .iter_mut()
            .find(|(property_name, _)| property_name == name)
        {
            *existing = value.to_string();
        } else {
            properties.push((name.to_string(), value.to_string()));
        }
        properties.sort_by(|left, right| left.0.cmp(&right.0));
        let state = chunk_nbt::block_state(self.block.as_ref(), &properties);
        Self {
            is_air: self.is_air,
            block: self.block.clone(),
            block_state_id: state.id,
            properties: Arc::from(state.properties),
        }
    }
}

#[derive(Debug, Clone)]
struct FlatLayer {
    block: String,
    block_state_id: i32,
    properties: Vec<(String, String)>,
    is_air: bool,
}

#[derive(Deserialize)]
struct FlatPresetFile {
    settings: FlatPresetSettings,
}

#[derive(Deserialize)]
struct FlatPresetSettings {
    biome: String,
    layers: Vec<FlatPresetLayer>,
}

#[derive(Deserialize)]
struct FlatPresetLayer {
    block: String,
    height: usize,
}

fn load_flat_preset(preset: &str) -> crate::error::Result<FlatSettings> {
    let path = flat_preset_path(preset)?;
    let content =
        std::fs::read_to_string(&path).map_err(|source| crate::error::WorldError::io_context(format!("read {}", path.display()), source))?;
    let preset: FlatPresetFile =
        serde_json::from_str(&content)
            .map_err(|source| crate::error::WorldError::msg(format!("parse {}: {source}", path.display())))?;
    Ok(FlatSettings {
        biome: preset.settings.biome,
        layers: preset
            .settings
            .layers
            .into_iter()
            .map(|layer| FlatLayerSetting {
                block: layer.block,
                height: layer.height,
            })
            .collect(),
    })
}

fn load_noise_settings(preset: &str, seed: i64) -> crate::error::Result<NoiseSettings> {
    let path = noise_settings_path(preset)?;
    let content =
        std::fs::read_to_string(&path).map_err(|source| crate::error::WorldError::io_context(format!("read {}", path.display()), source))?;
    let value: serde_json::Value =
        serde_json::from_str(&content)
            .map_err(|source| crate::error::WorldError::msg(format!("parse {}: {source}", path.display())))?;
    let noise = value
        .get("noise")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| crate::error::WorldError::msg("noise settings missing noise object"))?;
    let min_y = json_i32(noise.get("min_y")).unwrap_or(WORLD_MIN_Y);
    let height =
        json_i32(noise.get("height")).unwrap_or(super::WORLD_SECTION_COUNT as i32 * SECTION_HEIGHT);
    let sea_level = json_i32(value.get("sea_level")).unwrap_or(63);
    let default_block = json_block_name(value.get("default_block")).unwrap_or("minecraft:stone");
    let default_fluid = json_block_name(value.get("default_fluid")).unwrap_or("minecraft:water");
    let surface_rules = vanilla_noise::OverworldSurfaceRules::new(seed);
    let noise_kind = vanilla_noise::OverworldNoiseKind::from_preset(preset);

    Ok(NoiseSettings {
        seed,
        min_y,
        height,
        sea_level,
        air_block: BlockLayer::new("minecraft:air"),
        bedrock_block: BlockLayer::new("minecraft:bedrock"),
        default_block: BlockLayer::new(default_block),
        default_fluid: BlockLayer::new(default_fluid),
        lava_block: BlockLayer::new("minecraft:lava"),
        surface_block: BlockLayer::new("minecraft:grass_block"),
        subsurface_block: BlockLayer::new("minecraft:dirt"),
        deepslate_block: BlockLayer::new("minecraft:deepslate"),
        podzol_block: BlockLayer::new("minecraft:podzol"),
        coarse_dirt_block: BlockLayer::new("minecraft:coarse_dirt"),
        mycelium_block: BlockLayer::new("minecraft:mycelium"),
        calcite_block: BlockLayer::new("minecraft:calcite"),
        gravel_block: BlockLayer::new("minecraft:gravel"),
        sand_block: BlockLayer::new("minecraft:sand"),
        sandstone_block: BlockLayer::new("minecraft:sandstone"),
        packed_ice_block: BlockLayer::new("minecraft:packed_ice"),
        ice_block: BlockLayer::new("minecraft:ice"),
        snow_block: BlockLayer::new("minecraft:snow_block"),
        powder_snow_block: BlockLayer::new("minecraft:powder_snow"),
        mud_block: BlockLayer::new("minecraft:mud"),
        water_block: BlockLayer::new("minecraft:water"),
        terracotta_block: BlockLayer::new("minecraft:terracotta"),
        orange_terracotta_block: BlockLayer::new("minecraft:orange_terracotta"),
        white_terracotta_block: BlockLayer::new("minecraft:white_terracotta"),
        yellow_terracotta_block: BlockLayer::new("minecraft:yellow_terracotta"),
        brown_terracotta_block: BlockLayer::new("minecraft:brown_terracotta"),
        red_terracotta_block: BlockLayer::new("minecraft:red_terracotta"),
        light_gray_terracotta_block: BlockLayer::new("minecraft:light_gray_terracotta"),
        red_sand_block: BlockLayer::new("minecraft:red_sand"),
        copper_ore_block: BlockLayer::new("minecraft:copper_ore"),
        raw_copper_block: BlockLayer::new("minecraft:raw_copper_block"),
        granite_block: BlockLayer::new("minecraft:granite"),
        deepslate_iron_ore_block: BlockLayer::new("minecraft:deepslate_iron_ore"),
        raw_iron_block: BlockLayer::new("minecraft:raw_iron_block"),
        tuff_block: BlockLayer::new("minecraft:tuff"),
        biome: "minecraft:plains".to_string(),
        density: TerrainDensity::overworld(seed, noise_kind),
        surface_rules,
        aquifer: vanilla_noise::OverworldAquifer::new(seed, sea_level),
        ore_veins: vanilla_noise::OreVeinNoise::new(seed),
        carvers: VanillaCarvers::new(seed),
        ore_features: OverworldOreFeatures::new(seed),
        lava_lake_fluid_block: BlockLayer::new("minecraft:lava"),
        lava_lake_barrier_block: BlockLayer::new("minecraft:stone"),
        cave_air_block: BlockLayer::new("minecraft:cave_air"),
        feature_source_cache: FeatureSourceCache::default(),
    })
}

fn flat_preset_path(preset: &str) -> crate::error::Result<PathBuf> {
    let preset = preset.trim();
    let name = preset
        .strip_prefix("minecraft:")
        .or_else(|| preset.strip_prefix(':'))
        .unwrap_or(preset)
        .trim_matches('/');
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(crate::error::WorldError::msg(format!(
        "invalid flat preset name: {preset}"
    )));
    }
    Ok(workspace_root()
        .join(FLAT_PRESET_ROOT)
        .join(name)
        .with_extension("json"))
}

fn noise_settings_path(preset: &str) -> crate::error::Result<PathBuf> {
    let name = resource_path_name(preset)?;
    Ok(workspace_root()
        .join(NOISE_SETTINGS_ROOT)
        .join(name)
        .with_extension("json"))
}

fn resource_path_name(value: &str) -> crate::error::Result<String> {
    let value = value.trim();
    let name = value
        .strip_prefix("minecraft:")
        .or_else(|| value.strip_prefix(':'))
        .unwrap_or(value)
        .trim_matches('/');
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(crate::error::WorldError::msg(format!(
        "invalid resource name: {value}"
    )));
    }
    Ok(name.to_string())
}

fn expand_layers(settings: Vec<FlatLayerSetting>) -> Vec<FlatLayer> {
    let mut layers = Vec::new();
    for layer in settings {
        let block = normalize_identifier(&layer.block);
        let block_state = chunk_nbt::default_block_state(&block);
        let is_air = is_air_block(&block);
        for _ in 0..layer.height {
            layers.push(FlatLayer {
                block: block.clone(),
                block_state_id: block_state.id,
                properties: block_state.properties.clone(),
                is_air,
            });
        }
        if layers.len() >= super::WORLD_SECTION_COUNT * SECTION_HEIGHT as usize {
            layers.truncate(super::WORLD_SECTION_COUNT * SECTION_HEIGHT as usize);
            break;
        }
    }
    layers
}

fn flat_chunk_root(layers: &[FlatLayer], biome: &str) -> Tag {
    compound_tag([
        ("sections", sections_tag(layers, biome)),
        ("Heightmaps", heightmaps_tag(layers)),
    ])
}

fn noise_chunk_root(chunk: &NoiseChunkBlocks, _biome: &str) -> Tag {
    compound_tag([
        ("sections", noise_sections_tag(chunk)),
        ("Heightmaps", noise_heightmaps_tag(chunk)),
    ])
}

fn append_block_entities_to_chunk_root(root: &mut Tag, chunk: &NoiseChunkBlocks) {
    let Tag::Compound(fields) = root else {
        return;
    };
    let mut fields = (**fields).clone();
    let block_entities = chunk
        .block_entities
        .iter()
        .map(block_entity_tag)
        .collect::<Vec<_>>();
    fields.insert(
        "block_entities".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: block_entities.len() as i32,
            },
            Arc::from(block_entities),
        ),
    );
    *root = Tag::Compound(Arc::new(fields));
}

fn block_entity_tag(entity: &GeneratedBlockEntity) -> Tag {
    let mut fields = match &entity.nbt {
        Tag::Compound(fields) => (**fields).clone(),
        _ => HashMap::new(),
    };
    fields.insert(
        "id".to_string(),
        Tag::String(Arc::from(block_entity_type_name(entity.entity_type))),
    );
    fields.insert("x".to_string(), Tag::Int(entity.position.0));
    fields.insert("y".to_string(), Tag::Int(entity.position.1));
    fields.insert("z".to_string(), Tag::Int(entity.position.2));
    Tag::Compound(Arc::new(fields))
}

fn block_entity_type_name(entity_type: i32) -> &'static str {
    match entity_type {
        CHEST_BLOCK_ENTITY_TYPE_ID => "minecraft:chest",
        MOB_SPAWNER_BLOCK_ENTITY_TYPE_ID => "minecraft:mob_spawner",
        BEEHIVE_BLOCK_ENTITY_TYPE_ID => "minecraft:beehive",
        BRUSHABLE_BLOCK_ENTITY_TYPE_ID => "minecraft:brushable_block",
        _ => "minecraft:chest",
    }
}

fn sections_tag(layers: &[FlatLayer], biome: &str) -> Tag {
    let mut sections = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        let start_layer = ((section_y * SECTION_HEIGHT) - WORLD_MIN_Y) as usize;
        let end_layer = start_layer + SECTION_HEIGHT as usize;
        let section_layers = &layers[start_layer.min(layers.len())..end_layer.min(layers.len())];
        if section_layers.iter().all(|layer| layer.is_air) {
            continue;
        }
        sections.push(section_tag(section_y, section_layers, biome));
    }

    Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: sections.len() as i32,
        },
        Arc::from(sections),
    )
}

fn section_tag(section_y: i32, layers: &[FlatLayer], biome: &str) -> Tag {
    compound_tag([
        ("Y", Tag::Byte(section_y as i8)),
        ("block_states", block_states_tag(layers)),
        ("biomes", biomes_tag(biome)),
    ])
}

fn noise_sections_tag(chunk: &NoiseChunkBlocks) -> Tag {
    let mut sections = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        if noise_section_is_air(chunk, section_y) {
            continue;
        }
        sections.push(compound_tag([
            ("Y", Tag::Byte(section_y as i8)),
            ("block_states", noise_block_states_tag(chunk, section_y)),
            ("biomes", noise_biomes_tag(chunk, section_y)),
        ]));
    }

    Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: sections.len() as i32,
        },
        Arc::from(sections),
    )
}

fn noise_section_is_air(chunk: &NoiseChunkBlocks, section_y: i32) -> bool {
    let start_y = section_y * SECTION_HEIGHT;
    for y in 0..SECTION_HEIGHT as usize {
        let world_y = start_y + y as i32;
        if !(WORLD_MIN_Y..=WORLD_MAX_Y).contains(&world_y) {
            continue;
        }
        let index = (world_y - WORLD_MIN_Y) as usize;
        for z in 0..16 {
            for x in 0..16 {
                if !chunk.column(x, z).blocks[index].is_air {
                    return false;
                }
            }
        }
    }
    true
}

fn noise_block_states_tag(chunk: &NoiseChunkBlocks, section_y: i32) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_block = HashMap::<String, usize>::new();
    let mut values = vec![0_i32; BLOCK_ENTRY_COUNT];
    let start_y = section_y * SECTION_HEIGHT;

    for local_y in 0..SECTION_HEIGHT as usize {
        let world_y = start_y + local_y as i32;
        for z in 0..16 {
            for x in 0..16 {
                let layer = if (WORLD_MIN_Y..=WORLD_MAX_Y).contains(&world_y) {
                    let y_index = (world_y - WORLD_MIN_Y) as usize;
                    &chunk.column(x, z).blocks[y_index]
                } else {
                    continue;
                };
                let key = block_state_palette_key(&layer.block, &layer.properties);
                let next_index = palette.len();
                let palette_index = *index_by_block.entry(key).or_insert_with(|| {
                    palette.push(block_state_tag(&layer.block, &layer.properties));
                    next_index
                });
                values[(local_y * 16 + z) * 16 + x] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = palette_storage_bits(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
}

fn block_states_tag(layers: &[FlatLayer]) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_block = HashMap::<String, usize>::new();
    let mut values = vec![0_i32; BLOCK_ENTRY_COUNT];
    let air = chunk_nbt::default_block_state("minecraft:air");

    for local_y in 0..SECTION_HEIGHT as usize {
        let (block, properties) = layers
            .get(local_y)
            .map(|layer| (layer.block.as_str(), layer.properties.as_slice()))
            .unwrap_or(("minecraft:air", air.properties.as_slice()));
        let key = block_state_palette_key(block, properties);
        let next_index = palette.len();
        let palette_index = *index_by_block.entry(key).or_insert_with(|| {
            palette.push(block_state_tag(block, properties));
            next_index
        });

        for z in 0..16 {
            for x in 0..16 {
                values[(local_y * 16 + z) * 16 + x] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = palette_storage_bits(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
}

fn biomes_tag(biome: &str) -> Tag {
    paletted_container(vec![Tag::String(Arc::from(biome.to_string()))], None)
}

fn noise_biomes_tag(chunk: &NoiseChunkBlocks, section_y: i32) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_biome = HashMap::<&'static str, usize>::new();
    let mut values = vec![0_i32; 4 * 4 * 4];

    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let biome = chunk.biome(section_y, x, y, z);
                let next_index = palette.len();
                let palette_index = *index_by_biome.entry(biome).or_insert_with(|| {
                    palette.push(Tag::String(Arc::from(biome)));
                    next_index
                });
                values[(x * 4 + y) * 4 + z] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = ceil_log2(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
}

fn heightmaps_tag(layers: &[FlatLayer]) -> Tag {
    let height = flat_first_available_height(layers);
    let packed = pack_fixed_long_values(&vec![height; HEIGHTMAP_ENTRY_COUNT], HEIGHTMAP_BITS)
        .into_iter()
        .map(|value| value as i64)
        .collect::<Vec<_>>();
    compound_tag([
        ("WORLD_SURFACE", Tag::LongArray(Arc::from(packed.clone()))),
        ("MOTION_BLOCKING", Tag::LongArray(Arc::from(packed.clone()))),
        (
            "MOTION_BLOCKING_NO_LEAVES",
            Tag::LongArray(Arc::from(packed)),
        ),
    ])
}

fn noise_heightmaps_tag(chunk: &NoiseChunkBlocks) -> Tag {
    let mut values = vec![0_i32; HEIGHTMAP_ENTRY_COUNT];
    for z in 0..16 {
        for x in 0..16 {
            values[z * 16 + x] = chunk.column(x, z).first_available_height;
        }
    }
    let packed = pack_fixed_long_values(&values, HEIGHTMAP_BITS)
        .into_iter()
        .map(|value| value as i64)
        .collect::<Vec<_>>();
    compound_tag([
        ("WORLD_SURFACE", Tag::LongArray(Arc::from(packed.clone()))),
        ("MOTION_BLOCKING", Tag::LongArray(Arc::from(packed.clone()))),
        (
            "MOTION_BLOCKING_NO_LEAVES",
            Tag::LongArray(Arc::from(packed)),
        ),
    ])
}

fn flat_first_available_height(layers: &[FlatLayer]) -> i32 {
    let Some(highest) = layers.iter().rposition(|layer| !layer.is_air) else {
        return 0;
    };
    let height = WORLD_MIN_Y + highest as i32 + 1;
    (height - WORLD_MIN_Y).clamp(0, WORLD_MAX_Y - WORLD_MIN_Y + 1)
}

fn block_state_tag(name: &str, properties: &[(String, String)]) -> Tag {
    let mut fields = HashMap::new();
    fields.insert("Name".to_string(), Tag::String(Arc::from(name.to_string())));
    if !properties.is_empty() {
        fields.insert(
            "Properties".to_string(),
            Tag::Compound(Arc::new(
                properties
                    .iter()
                    .map(|(key, value)| (key.clone(), Tag::String(Arc::from(value.clone()))))
                    .collect(),
            )),
        );
    }
    Tag::Compound(Arc::new(fields))
}

fn block_state_palette_key(name: &str, properties: &[(String, String)]) -> String {
    let mut key = name.to_string();
    key.push('|');
    for (name, value) in properties {
        key.push_str(name);
        key.push('=');
        key.push_str(value);
        key.push(';');
    }
    key
}

fn paletted_container(palette: Vec<Tag>, data: Option<Vec<i64>>) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "palette".to_string(),
        Tag::List(
            ListHeader {
                tag_id: palette.first().map(Tag::tag_id).unwrap_or(tag_id::END),
                length: palette.len() as i32,
            },
            Arc::from(palette),
        ),
    );
    if let Some(data) = data {
        fields.insert("data".to_string(), Tag::LongArray(Arc::from(data)));
    }
    Tag::Compound(Arc::new(fields))
}

fn compound_tag<I>(fields: I) -> Tag
where
    I: IntoIterator<Item = (&'static str, Tag)>,
{
    Tag::Compound(Arc::new(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    ))
}

fn palette_storage_bits(palette_len: usize) -> usize {
    ceil_log2(palette_len).max(4)
}

fn pack_fixed_long_values(values: &[i32], bits: usize) -> Vec<u64> {
    let values_per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];

    for (index, value) in values.iter().enumerate() {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= ((*value as u64) & mask) << bit_offset;
    }

    packed
}

fn normalize_identifier(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn is_air_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

fn json_i32(value: Option<&serde_json::Value>) -> Option<i32> {
    value
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn json_block_name(value: Option<&serde_json::Value>) -> Option<&str> {
    value?
        .get("Name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
